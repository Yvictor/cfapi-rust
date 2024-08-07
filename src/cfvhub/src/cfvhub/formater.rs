use serde::Serialize;
use snafu::{prelude::Snafu, ResultExt};

#[derive(Debug, Snafu)]
pub enum FormatError {
    #[snafu(display("Error Eecode Json: {}", source))]
    Json { source: serde_json::Error },
    #[snafu(display("Error Eecode Yaml: {}", source))]
    Yaml{ source: serde_yaml::Error },
    #[snafu(display("Error Eecode Toml: {}", source))]
    Toml{ source: toml::ser::Error },
    #[snafu(display("Error Eecode MessagePack: {}", source))]
    MessagePack{ source: rmp_serde::encode::Error },
}

pub trait FormaterExt<In> 
where
    In: Serialize,
{
    fn format(&self, input: &In) -> Result<Formated, FormatError>;
    fn content_type(&self) -> &str;
}

pub enum Formated {
    String(String),
    Bytes(Vec<u8>),
}

#[derive(Debug, Default)]
pub struct JsonFormater;

impl<I> FormaterExt<I> for JsonFormater
where
    I: Serialize,
{
    fn format(&self,input: &I) -> Result<Formated, FormatError> {
        Ok(Formated::String(serde_json::to_string(input).context(JsonSnafu)?))
    }
    fn content_type(&self) -> &str {
        "json"
    }
}


#[derive(Debug, Default)]
pub struct YamlFormater;

impl<I> FormaterExt<I> for YamlFormater
where
    I: Serialize,
{
    fn format(&self,input: &I) -> Result<Formated, FormatError> {
        Ok(Formated::String(serde_yaml::to_string(input).context(YamlSnafu)?))
    }
    fn content_type(&self) -> &str {
        "yaml"
    }
}

#[derive(Debug, Default)]
pub struct TomlFormater;

impl<I> FormaterExt<I> for TomlFormater
where
    I: Serialize,
{
    fn format(&self,input: &I) -> Result<Formated, FormatError> {
        Ok(Formated::String(toml::to_string(input).context(TomlSnafu)?))
    }
    fn content_type(&self) -> &str {
        "toml"
    }
}

#[derive(Debug, Default)]
pub struct MessagePackFormater;

impl<I> FormaterExt<I> for MessagePackFormater
where
    I: Serialize,
{
    fn format(&self,input: &I) -> Result<Formated, FormatError> {
        Ok(Formated::Bytes(rmp_serde::to_vec(input).context(MessagePackSnafu)?))
    }
    fn content_type(&self) -> &str {
        "msgpack"
    }
}
