use clap::Parser;

/// This is a ranked voting tabulation program.
#[derive(Parser, Debug, Clone)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    /// (file path, optional) The file containing the election data. (Only JSON election descriptions are currently supported)
    /// For more information about the file format, read the documentation at
    #[clap(short, long, value_parser)]
    pub config: Option<String>,
    /// (file path) A reference file containing the outcome of an election in JSON format. If provided, timrcv will
    /// check that the tabulated output matches the reference.
    #[clap(short, long, value_parser)]
    pub reference: Option<String>,

    /// (file path, 'stdout' or empty) If specified, the summary of the election will be written in JSON format to the given
    /// location. Setting this option overrides the path that may be specified with the --config option.
    #[clap(short, long, value_parser)]
    pub out: Option<String>,

    /// (file path or empty) If specified, the summary of the election will be written in JSON format to the given
    /// location. Setting this option overrides what may be specified with the --data option.
    #[clap(short, long, value_parser)]
    pub input: Option<String>,

    /// (default csv) The type of the input. See documentation for all the input types.
    #[clap(long, value_parser)]
    pub input_type: Option<String>,

    /// (list of comma-separated values or not specified) If specified, the list of labels for the ranks. This is useful for
    /// Likert-like styles of inputs in which there is no natural order. It should correspond to the entries in the first row
    /// of the input.
    #[clap(long, value_parser)]
    pub choices: Option<Vec<String>>,

    /// (default Form1) When using an Excel file, indicates the name of the worksheet to use.
    #[clap(long, value_parser)]
    pub excel_worksheet_name: Option<String>,

    // Other arguments
    /// If passed as an argument, will turn on verbose logging to the standard output.
    #[clap(long, takes_value = false)]
    pub verbose: bool,
}
