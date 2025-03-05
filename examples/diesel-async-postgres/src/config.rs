pub struct Config {
    pub database_url: String,
}

impl Config {
    pub fn new() -> Config {
        let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        Config {
            database_url: db_url,
        }
    }
}
