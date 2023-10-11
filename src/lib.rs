//! Provides [`SampleLogger`], which can be inserted into a plugin to keep track of internal values 
//! every sample time. The CSV can then be plotted in a spreadshot program or with e.g. matplotlib.
//! This crate has an `disable` feature which when turned on disables all the code that might slow
//! down the plugin. This way it's possible to insert the logging in a plugin but build it in a 
//! production mode as well where the logging doesn't cause performance issues.

extern crate csv;
use std::{collections::HashMap, error::Error, fs::File};


/// Reads a CSV file and converts it into a hashmap where each key corresponds to a column header
/// and each value is a vector of floats representing the audio data. This function exists to
/// allow you to write your own plotters/handlers of the data written to the output of this library,
/// so in a sense this function is the inverse of the result that [`SampleLogger`] produces.
///
/// # Arguments
///
/// * `filename` - A string representing the path to the CSV file.
///
/// # Returns
///
/// * `Result<HashMap<String, Vec<f32>>, Box<dyn Error>>` - On success, returns a HashMap where each
///   key is a column header from the CSV and each value is a vector of floats representing the audio
///   data in that column. On failure, returns an error.
///
/// # Errors
///
/// This function will return an error if:
///
/// * The file cannot be opened.
/// * There is an error reading the CSV headers or records.
/// * There is a parse error while reading CSV data.
///
/// # Panics
///
/// This function might panic if a parse error occurs while reading CSV data.
pub fn read_csv_as_audio_data(
    filename: String,
) -> Result<HashMap<String, Vec<f32>>, Box<dyn Error>> {
    let mut reader = csv::Reader::from_reader(File::open(filename.as_str())?);
    let headers = reader.headers()?.clone();
    let mut data = HashMap::new();

    for record in reader.records() {
        let record = record?;
        let row: Vec<f32> = record
            .iter()
            .map(|x| x.parse().expect("parse error on reading csv"))
            .collect();

        for (i, header) in headers.iter().enumerate() {
            data.entry(String::from(header))
                .or_insert(Vec::new())
                .push(row[i] as f32);
        }
    }

    Ok(data)
}

/// Logs the operation of an audio plugin to a CSV file. Every line of this CSV is either a sample, or some other value
/// during operation of the plugin at exactly that time. Enforces that every column of the CSV is the same length.
///
/// For example, one column could be the sample before processing, and another after processing. Additionally columns could be
/// filled with (if applicable) the envelope at that sample, attack/release information, the absolute value of a sample, the
/// current gain parameter, or whatever else might be interesting to look at in detail.
///
/// It is mandatory to have at least one field named 'sample', since that's what's counted to determine that the logging should
/// stop.
/// 
/// A minimal structure to use this library in a VST3 plugin (e.g. using [nih-plug](https://github.com/robbert-vdh/nih-plug))
/// requires:
/// 
/// * A [`SampleLogger`] (likely on the struct that implements `Plugin`).
/// * For every sample that the plugin handles, calls to [`SampleLogger::write`], at least one of which must be named 'sample'.
/// * on deactivation of the plugin, or termination of the program, a call to [`SampleLogger::write_debug_values`].
/// 
/// A project using this crate can be found [here](https://github.com/PietPtr/compressor).
///
/// # Fields
///
/// * `debug_values`: A map where each key corresponds to an identifier and each value is a vector of floats representing the data.
/// * `samples_seen`: A counter for the number of samples seen.
/// * `quit_after_n_samples`: An optional field specifying the number of samples after which logging should stop.
/// * `output_file`: The name of the file where the logged data will be written to.
pub struct SampleLogger {
    debug_values: HashMap<String, Vec<f32>>,
    samples_seen: u64,
    quit_after_n_samples: Option<u64>,
    output_file: String,
}

impl SampleLogger {
    /// Creates a new `SampleLogger` instance with sensible defaults.
    ///
    /// # Arguments
    ///
    /// * `output_file`: The name of the file where the logged data will be written to.
    ///
    /// # Returns
    ///
    /// * `SampleLogger`: The newly created `SampleLogger` instance.
    pub fn new(output_file: String) -> Self {
        Self {
            debug_values: HashMap::new(),
            samples_seen: 0,
            quit_after_n_samples: None,
            output_file,
        }
    }

    /// Logs a single sample or skips if enough samples have been logged. Also applies the enforcement of every
    /// column having the same length.
    ///
    /// # Arguments
    ///
    /// * `key`: A string slice representing the identifier of the stream of samples that this sample should be written to.
    /// * `value`: A float representing the sample value.
    ///
    /// # Returns
    ///
    /// * `Result<(), &'static str>`: Returns `Ok(())` if the sample is logged successfully or if logging is disabled.
    ///   Returns `Err` with a static string describing the error otherwise.
    pub fn write(&mut self, key: &str, value: f32) -> Result<(), &'static str> {
        if !cfg!(feature = "disabled") {
            if !self.is_logging_active() {
                return Ok(()); // Don't write anything if we've seen enough samples.
            }

            self.debug_values
                .entry(String::from(key))
                .or_insert(Vec::new())
                .push(value);

            if key == "sample" {
                // TODO: Could count all values, since after one iteration the amount of keys is known, and would remove the mandatory 'sample' key
                self.samples_seen += 1;
            }

            return self.is_logged_correctly();
        } else {
            Ok(())
        }
    }

    /// Sets the `quit_after_n_samples` field. Useful if the plugin is applied to a large audio file and you'd like the
    /// CSV to remain tiny.
    ///
    /// # Arguments
    ///
    /// * `samples`: A float representing the number of samples after which logging should stop.
    pub fn set_quit_after_n_samples(&mut self, samples: u64) {
        self.quit_after_n_samples = Some(samples as u64);
    }

    /// Determines whether the logging is still active based on the number of samples seen and 
    /// the optional `quit_after_n_samples` field. Returns true if quit_after_n_samples is None.
    ///
    /// # Returns
    ///
    /// * `bool`: Returns `true` if logging is still active (i.e., the number of samples seen is 
    ///   less than the optional `quit_after_n_samples` field or if `quit_after_n_samples` is `None`).
    ///   Returns `false` otherwise.
    pub fn is_logging_active(&self) -> bool {
        match self.quit_after_n_samples {
            Some(limit) => self.samples_seen < limit,
            None => true,
        }
    }

    /// Checks whether the logged data is correctly formatted, this function enforces that the 'sample' key is present
    /// and that every column stays of the same size.
    ///
    /// # Returns
    ///
    /// * `Result<(), &'static str>`: Returns `Ok(())` if the logged data is correctly formatted.
    ///   Returns `Err` either when an element causes the columns to not be the same length anymore
    ///   or when the 'sample' key is not present.
    fn is_logged_correctly(&self) -> Result<(), &'static str> {
        let lengths: Vec<usize> = self
            .debug_values
            .values()
            .into_iter()
            .map(|vec| vec.len())
            .collect();

        // Checks whether all lists have n or n+1 elements.
        let n = lengths.iter().sum::<usize>() / lengths.len();
        if !lengths.iter().all(|&elem| elem == n || elem == n + 1) {
            return Err("Element added to list caused imbalance.");
        }

        if n > 1 && !self.debug_values.keys().any(|elem| elem.eq("sample")) {
            dbg!(self.debug_values.keys());
            return Err("First sample iteration has been added but no key 'sample' is present.");
        }

        Ok(())
    }

    /// Writes the logged data to the specified output file. Call this on shutdown of the plugin.
    ///
    /// # Returns
    ///
    /// * `Result<(), Box<dyn Error>>`: Returns `Ok(())` if the data is written successfully or if logging is disabled.
    ///    Propagates any IO errors otherswise.
    pub fn write_debug_values(&mut self) -> Result<(), Box<dyn Error>> {
        if !cfg!(feature = "disabled") {
            self.is_logged_correctly()?;

            let max_len = self
                .debug_values
                .values()
                .map(|v| v.len())
                .max()
                .unwrap_or(0);

            let file = File::create(self.output_file.as_str())?;
            let mut writer = csv::Writer::from_writer(file);

            writer.write_record(self.debug_values.keys())?;

            for i in 0..max_len {
                let mut record = csv::StringRecord::new();
                for value in self.debug_values.values() {
                    let entry = value.get(i).map(|v| v.to_string()).unwrap_or(String::new());
                    record.push_field(entry.as_str());
                }
                writer.write_record(&record)?;
            }
        }

        Ok(())
    }
}
