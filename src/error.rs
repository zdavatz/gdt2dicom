use std::fmt;

#[derive(Debug)]
pub enum G2DError {
    IoError(std::io::Error),
    NotifyError(notify::Error),
    GdtError(GdtError),
    XmlReaderError(xml::reader::Error),
    XmlWriterError(xml::writer::Error),
}

#[derive(Debug)]
pub enum GdtError {
    FieldIdentifierNotNumber(String, std::num::ParseIntError),
    LineTooShort(String),
    LineNotFound(String),
    NumberExpected(String, std::num::ParseIntError),
    InvalidValue(String, String),
}

impl From<std::io::Error> for G2DError {
    fn from(error: std::io::Error) -> Self {
        G2DError::IoError(error)
    }
}

impl From<notify::Error> for G2DError {
    fn from(error: notify::Error) -> Self {
        G2DError::NotifyError(error)
    }
}

impl From<GdtError> for G2DError {
    fn from(error: GdtError) -> Self {
        G2DError::GdtError(error)
    }
}

impl From<xml::reader::Error> for G2DError {
    fn from(error: xml::reader::Error) -> Self {
        G2DError::XmlReaderError(error)
    }
}

impl From<xml::writer::Error> for G2DError {
    fn from(error: xml::writer::Error) -> Self {
        G2DError::XmlWriterError(error)
    }
}

impl fmt::Display for G2DError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            G2DError::IoError(e) => write!(f, "IO: {}", e),
            G2DError::NotifyError(e) => write!(f, "NotifyError: {}", e),
            G2DError::GdtError(e) => write!(f, "GdtError: {}", e),
            G2DError::XmlReaderError(e) => write!(f, "XmlReaderError: {}", e),
            G2DError::XmlWriterError(e) => write!(f, "XmlWriterError: {}", e),
        }
    }
}

impl std::fmt::Display for GdtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
