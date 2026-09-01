use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorPickResult {
    request_id: String,
    color: Option<PickedColor>,
    cancelled: bool,
    error: Option<String>,
}

impl ColorPickResult {
    pub fn success(request_id: String, red: u8, green: u8, blue: u8) -> Self {
        Self {
            request_id,
            color: Some(PickedColor::new(red, green, blue)),
            cancelled: false,
            error: None,
        }
    }

    pub fn cancelled(request_id: String) -> Self {
        Self {
            request_id,
            color: None,
            cancelled: true,
            error: None,
        }
    }

    pub fn error(request_id: String, error: impl Into<String>) -> Self {
        Self {
            request_id,
            color: None,
            cancelled: false,
            error: Some(error.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PickedColor {
    hex: String,
    red: u8,
    green: u8,
    blue: u8,
}

impl PickedColor {
    fn new(red: u8, green: u8, blue: u8) -> Self {
        Self {
            hex: format!("#{red:02X}{green:02X}{blue:02X}"),
            red,
            green,
            blue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_uppercase_hex_with_zero_padding() {
        let result = ColorPickResult::success("c-1".to_owned(), 1, 10, 255);
        assert_eq!(
            result.color,
            Some(PickedColor {
                hex: "#010AFF".to_owned(),
                red: 1,
                green: 10,
                blue: 255,
            })
        );
    }
}
