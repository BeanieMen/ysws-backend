use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize)]
pub struct RegisterAttendanceRequest {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AttendanceRegistrationResponse {
    pub registration_id: Uuid,
    pub event_id: Uuid,
    pub status: String,
    pub participant_id: Option<String>,
}
