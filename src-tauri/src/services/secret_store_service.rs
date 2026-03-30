use std::sync::Arc;

#[cfg(test)]
use std::{
    collections::HashMap,
    sync::Mutex,
};

use keyring::{Entry, Error as KeyringError};

use crate::error::{AppError, AppResult};

const KEYRING_SERVICE_NAME: &str = "com.postnot.app.environment-variable";

pub trait SecretStore: Send + Sync {
    fn get_environment_variable_secret(
        &self,
        environment_id: &str,
        variable_id: &str,
    ) -> AppResult<Option<String>>;
    fn set_environment_variable_secret(
        &self,
        environment_id: &str,
        variable_id: &str,
        value: &str,
    ) -> AppResult<()>;
    fn delete_environment_variable_secret(
        &self,
        environment_id: &str,
        variable_id: &str,
    ) -> AppResult<()>;
}

pub fn default_secret_store() -> Arc<dyn SecretStore> {
    Arc::new(KeyringSecretStore)
}

#[derive(Debug, Default)]
pub struct KeyringSecretStore;

impl SecretStore for KeyringSecretStore {
    fn get_environment_variable_secret(
        &self,
        environment_id: &str,
        variable_id: &str,
    ) -> AppResult<Option<String>> {
        let entry = environment_secret_entry(environment_id, variable_id)?;

        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(keyring_error("read", error)),
        }
    }

    fn set_environment_variable_secret(
        &self,
        environment_id: &str,
        variable_id: &str,
        value: &str,
    ) -> AppResult<()> {
        let entry = environment_secret_entry(environment_id, variable_id)?;
        entry
            .set_password(value)
            .map_err(|error| keyring_error("write", error))
    }

    fn delete_environment_variable_secret(
        &self,
        environment_id: &str,
        variable_id: &str,
    ) -> AppResult<()> {
        let entry = environment_secret_entry(environment_id, variable_id)?;

        match entry.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(keyring_error("delete", error)),
        }
    }
}

fn environment_secret_entry(environment_id: &str, variable_id: &str) -> AppResult<Entry> {
    let username = format!("{environment_id}:{variable_id}");
    Entry::new(KEYRING_SERVICE_NAME, &username).map_err(|error| keyring_error("initialize", error))
}

fn keyring_error(action: &str, error: KeyringError) -> AppError {
    AppError::Message(format!(
        "Secure secret storage is unavailable or failed to {action} a value. {error}"
    ))
}

#[cfg(test)]
#[derive(Debug, Default)]
pub struct InMemorySecretStore {
    values: Mutex<HashMap<(String, String), String>>,
}

#[cfg(test)]
impl InMemorySecretStore {
    pub fn with_secret(environment_id: &str, variable_id: &str, value: &str) -> Self {
        let mut values = HashMap::new();
        values.insert(
            (environment_id.to_string(), variable_id.to_string()),
            value.to_string(),
        );

        Self {
            values: Mutex::new(values),
        }
    }
}

#[cfg(test)]
impl SecretStore for InMemorySecretStore {
    fn get_environment_variable_secret(
        &self,
        environment_id: &str,
        variable_id: &str,
    ) -> AppResult<Option<String>> {
        Ok(self
            .values
            .lock()
            .map_err(|_| AppError::Message("Failed to access test secret store.".to_string()))?
            .get(&(environment_id.to_string(), variable_id.to_string()))
            .cloned())
    }

    fn set_environment_variable_secret(
        &self,
        environment_id: &str,
        variable_id: &str,
        value: &str,
    ) -> AppResult<()> {
        self.values
            .lock()
            .map_err(|_| AppError::Message("Failed to access test secret store.".to_string()))?
            .insert(
                (environment_id.to_string(), variable_id.to_string()),
                value.to_string(),
            );
        Ok(())
    }

    fn delete_environment_variable_secret(
        &self,
        environment_id: &str,
        variable_id: &str,
    ) -> AppResult<()> {
        self.values
            .lock()
            .map_err(|_| AppError::Message("Failed to access test secret store.".to_string()))?
            .remove(&(environment_id.to_string(), variable_id.to_string()));
        Ok(())
    }
}
