# LLAD
A tool to aid in debugging audio plugins at the sample level. Provides an interface and utility functions to run a VST3 plugin as defined in NIH-plug for a small amount of time and saves information during running to a CSV that can then be plotted. 

When using this tool you still need something to actually run the plugin, preferably something light weight that only runs the plugin on some given audio file, since this library intentionally closes the plugin after a given amount of samples to prevent the CSV from growing too large.