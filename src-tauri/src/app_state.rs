use std::sync::{Arc, Mutex};

use sqlx::SqlitePool;
use tokio::sync::watch;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    services::secret_store_service::SecretStore,
};

struct InFlightRequest {
    id: String,
    cancel_tx: watch::Sender<bool>,
}

pub struct AppState {
    db: SqlitePool,
    secret_store: Arc<dyn SecretStore>,
    in_flight_request: Mutex<Option<InFlightRequest>>,
}

impl AppState {
    pub fn new(db: SqlitePool, secret_store: Arc<dyn SecretStore>) -> Self {
        Self {
            db,
            secret_store,
            in_flight_request: Mutex::new(None),
        }
    }

    pub fn db(&self) -> &SqlitePool {
        &self.db
    }

    pub fn secret_store(&self) -> Arc<dyn SecretStore> {
        Arc::clone(&self.secret_store)
    }

    pub fn start_request(&self) -> AppResult<(String, watch::Receiver<bool>)> {
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

        Ok((request_id, cancel_rx))
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
}
