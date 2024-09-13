use wasm_bindgen::JsValue;

#[derive(Debug)]
pub enum AppError {
    AnyError(anyhow::Error),
    SerdeJsonError(serde_wasm_bindgen::Error),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::AnyError(err) => err.fmt(f),
            AppError::SerdeJsonError(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for AppError {}

impl From<AppError> for JsValue {
    fn from(value: AppError) -> JsValue {
        match value {
            AppError::AnyError(err) => JsValue::from_str(format!("any error: {err:?}").as_str()),
            AppError::SerdeJsonError(err) => {
                JsValue::from_str(format!("serde error: {err:?}").as_str())
            }
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        Self::AnyError(value)
    }
}
impl From<serde_wasm_bindgen::Error> for AppError {
    fn from(value: serde_wasm_bindgen::Error) -> Self {
        Self::SerdeJsonError(value)
    }
}
