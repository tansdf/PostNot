use std::sync::{Arc, Mutex};

use sqlx::SqlitePool;
use tauri_plugin_updater::Update;
use tokio::sync::watch;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    services::{response_body_service::ResponseBodyStore, secret_store_service::SecretStore},
};

struct InFlightRequest {
    id: String,
    cancel_tx: watch::Sender<bool>,
}

pub struct RequestGuard<'a> {
    state: &'a AppState,
    id: String,
}

impl RequestGuard<'_> {
    pub fn id(&self) -> &str {
        &self.id
    }
}

impl Drop for RequestGuard<'_> {
    fn drop(&mut self) {
        self.state.finish_request(&self.id);
    }
}

pub struct AppState {
    db: SqlitePool,
    secret_store: Arc<dyn SecretStore>,
    in_flight_request: Mutex<Option<InFlightRequest>>,
    pending_update: Mutex<Option<Update>>,
    response_bodies: ResponseBodyStore,
}

impl AppState {
    pub fn new(
        db: SqlitePool,
        secret_store: Arc<dyn SecretStore>,
        response_bodies: ResponseBodyStore,
    ) -> Self {
        Self {
            db,
            secret_store,
            in_flight_request: Mutex::new(None),
            pending_update: Mutex::new(None),
            response_bodies,
        }
    }

    pub fn response_bodies(&self) -> &ResponseBodyStore {
        &self.response_bodies
    }

    pub fn db(&self) -> &SqlitePool {
        &self.db
    }

    pub fn secret_store(&self) -> Arc<dyn SecretStore> {
        Arc::clone(&self.secret_store)
    }

    pub fn start_request(&self) -> AppResult<(RequestGuard<'_>, watch::Receiver<bool>)> {
        let mut in_flight_request = self
            .in_flight_request
            .lock()
            .map_err(|_| AppError::Message("Failed to access request state.".to_string()))?;

        if in_flight_request.is_some() {
            return Err(AppError::Message(
                "Another request is already in flight.".to_string(),
            ));
        }

        let request_id = Uuid::new_v4().to_string();
        let (cancel_tx, cancel_rx) = watch::channel(false);

        *in_flight_request = Some(InFlightRequest {
            id: request_id.clone(),
            cancel_tx,
        });

        Ok((
            RequestGuard {
                state: self,
                id: request_id,
            },
            cancel_rx,
        ))
    }

    pub fn finish_request(&self, request_id: &str) {
        if let Ok(mut in_flight_request) = self.in_flight_request.lock() {
            if in_flight_request
                .as_ref()
                .map(|request| request.id.as_str() == request_id)
                .unwrap_or(false)
            {
                *in_flight_request = None;
            }
        }
    }

    pub fn cancel_active_request(&self) -> AppResult<bool> {
        let in_flight_request = self
            .in_flight_request
            .lock()
            .map_err(|_| AppError::Message("Failed to access request state.".to_string()))?;

        if let Some(request) = in_flight_request.as_ref() {
            let _ = request.cancel_tx.send(true);
            return Ok(true);
        }

        Ok(false)
    }

    pub fn set_pending_update(&self, update: Update) -> AppResult<()> {
        let mut pending_update = self
            .pending_update
            .lock()
            .map_err(|_| AppError::Message("Failed to access updater state.".to_string()))?;

        *pending_update = Some(update);
        Ok(())
    }

    pub fn pending_update(&self) -> AppResult<Option<Update>> {
        let pending_update = self
            .pending_update
            .lock()
            .map_err(|_| AppError::Message("Failed to access updater state.".to_string()))?;

        Ok(pending_update.clone())
    }

    pub fn clear_pending_update(&self) -> AppResult<()> {
        let mut pending_update = self
            .pending_update
            .lock()
            .map_err(|_| AppError::Message("Failed to access updater state.".to_string()))?;

        *pending_update = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use sqlx::SqlitePool;

    use super::AppState;
    use crate::services::{
        response_body_service::ResponseBodyStore, secret_store_service::InMemorySecretStore,
    };

    fn test_state() -> AppState {
        AppState::new(
            SqlitePool::connect_lazy("sqlite::memory:").expect("create lazy test pool"),
            Arc::new(InMemorySecretStore::default()),
            ResponseBodyStore::new(PathBuf::from("test-response-bodies")),
        )
    }

    #[tokio::test]
    async fn request_guard_releases_matching_request_on_drop() {
        let state = test_state();
        let (guard, _) = state.start_request().expect("start request");
        drop(guard);
        assert!(state.start_request().is_ok());
    }

    #[tokio::test]
    async fn finishing_an_old_request_does_not_clear_a_new_request() {
        let state = test_state();
        let (first, _) = state.start_request().expect("first request");
        let first_id = first.id().to_owned();
        drop(first);
        let (_second, _) = state.start_request().expect("second request");
        state.finish_request(&first_id);
        assert!(state.start_request().is_err());
    }
}
