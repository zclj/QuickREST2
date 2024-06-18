/// Sources to load data from
pub enum DataSource {
    /// A URL to fetch data from
    HttpUrl(String),

    /// Local file path
    FilePath(std::path::PathBuf),
}

impl DataSource {
    // pub fn stream(self) {
    //     match self {
    //         DataSource::FilePath(path) => {
    //             load_file_path(path)
    //         },

    //         _ => todo!("Data source not implemented"),
    //     }
    // }
}

// pub enum SystemCommand {
//     LoadDataSource(DataSource),
// }
