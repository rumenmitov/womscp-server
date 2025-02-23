use std::{fs, io, path::{Path, PathBuf}};
use toml::Table;
use clap::{Parser, Subcommand};


#[derive(Parser)]
#[command(name = "womscp-server")]
#[command(version = "1.0")]
#[command(about = "Server that handles the WOMSCP.", long_about = None)]
pub struct Cli {
    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>
}

#[derive(Subcommand)]
pub enum Commands {
    /// initializes the server
    Init
}


static DEFAULT_CONFIG :&'static str = "config.toml";

pub struct ServerConfig {
    pub address :String,
    pub database :String,
    pub microcontroller_count :u16,
    pub sensors_per_microcontroller :u8
}


impl ServerConfig {
    fn default() -> Self {
        ServerConfig {
            address: "127.0.0.1:3000".to_string(),
            database: "sqlite:w_orchid.db".to_string(),
            microcontroller_count: 1,
            sensors_per_microcontroller: 2
        }
    }

    pub fn new() -> Self {
        // NOTE: Default values for server config.
        if !Path::new(DEFAULT_CONFIG).exists() {
            Self::default()
        } else {
            DEFAULT_CONFIG.try_into().unwrap()
        }
    }
}


impl TryFrom<&str> for ServerConfig {
    type Error = io::Error;

    fn try_from(file: &str) -> Result<Self, Self::Error> {
        PathBuf::from(file).try_into()
    }
}

impl TryFrom<PathBuf> for ServerConfig {
    type Error = io::Error;

    fn try_from(file: PathBuf) -> Result<Self, Self::Error> {
        // NOTE: Default values for server config.
        let mut server_config = Self::default();

        let contents = fs::read_to_string(file)?;
        let config = match contents.parse::<Table>() {
            Ok(_config) => _config,
            Err(e) => {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, e.message()))
            }
        };

        server_config.address = if let Some(_address) = config["address"].as_str() {
            String::from(_address)
        } else {
            server_config.address
        };

        server_config.database = if let Some(_database) = config["database"].as_str() {
            String::from(_database)
        } else {
            server_config.database
        };

        server_config.microcontroller_count = if let Some(_count) = config["microcontroller_count"].as_integer(){
            _count as u16
        } else {
            server_config.microcontroller_count
        };

        server_config.sensors_per_microcontroller = if let Some(_count) = 
            config["sensors_per_microcontroller"].as_integer() {
                _count as u8
            } else {
                server_config.sensors_per_microcontroller
            };

        Ok(server_config)
    }
}


pub async fn server_init(server_config :&ServerConfig) {
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&server_config.database)
        .create_if_missing(true);

    let conn = sqlx::SqlitePool::connect_with(options).await.unwrap();

    if let Err(e) = sqlx::query("
CREATE TABLE Microcontrollers(
       id INTEGER PRIMARY KEY AUTOINCREMENT);


CREATE TABLE Sensors(
	m_id INT NOT NULL,
    s_id INT NOT NULL,
    PRIMARY KEY (m_id, s_id),
	FOREIGN KEY (m_id) REFERENCES Microcontrollers(id) ON DELETE CASCADE);


CREATE TABLE SensorData(
       id INTEGER PRIMARY KEY AUTOINCREMENT,
       timepoint TEXT NOT NULL,
       m_id INT NOT NULL,
       s_id INT NOT NULL,
       sensor_type INT NOT NULL,
       sensor_data INT NOT NULL,
       dummy BOOLEAN NOT NULL,       
       FOREIGN KEY (m_id, s_id) REFERENCES Sensors(m_id, s_id) ON DELETE CASCADE,       
       FOREIGN KEY (m_id) REFERENCES Microcontrollers(id) ON DELETE CASCADE);
        "
    )
    .execute(&conn)
    .await 
    {
            panic!("Failed to create database tables.\n{:#?}", e);
    }

    for m_id in 0..server_config.microcontroller_count {
        if let Err(e) = sqlx::query(
            "INSERT INTO Microcontrollers VALUES($1)"
        )
            .bind(m_id)
            .execute(&conn)
            .await
        {
            panic!("Failed to insert into Microntrollers.\n{:#?}", e);
        }

        for s_id in 0..server_config.sensors_per_microcontroller {
            if let Err(e) = sqlx::query(
                "INSERT INTO Sensors VALUES($1, $2)"
            )
                .bind(m_id)
                .bind(s_id)
                .execute(&conn)
                .await
            {
                panic!("Failed to insert into Sensors, s_id={}, m_id={}.\n{:#?}", 
                    s_id, m_id, e);
            }
        }
    }

    conn.close().await;
}
