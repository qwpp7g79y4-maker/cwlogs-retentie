use std::fmt;

/// CloudWatch Logs class. Storage is priced per class. Ingestion is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Standard,
    InfrequentAccess,
    Delivery,
}

impl Class {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "STANDARD" => Some(Self::Standard),
            "INFREQUENT_ACCESS" => Some(Self::InfrequentAccess),
            "DELIVERY" => Some(Self::Delivery),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "STANDARD",
            Self::InfrequentAccess => "INFREQUENT_ACCESS",
            Self::Delivery => "DELIVERY",
        }
    }
}

impl fmt::Display for Class {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
