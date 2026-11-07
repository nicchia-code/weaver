//! The Tauri commands of General: the conversation and the decisions of the developer in it.

use std::sync::Arc;

use tauri::State;

use super::cards::{CardActionInput, ChatCards};
use super::service::General;
use super::wire::GeneralSnapshot;
use crate::error::{AppError, ErrorCode};
use crate::subagents::wire::{Step, SubAgentConversation, SubAgentsEvent};

fn refused(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::Refused, message)
}

/// The conversation and the state of the General of an open project.
#[tauri::command]
pub async fn general_snapshot(
    general: State<'_, Arc<General>>,
    project_id: String,
) -> Result<GeneralSnapshot, AppError> {
    general.snapshot(&project_id).map_err(refused)
}

/// `developer.SEND_MESSAGE`: omp starts when it is not running.
#[tauri::command]
pub async fn general_send_message(
    general: State<'_, Arc<General>>,
    project_id: String,
    text: String,
) -> Result<(), AppError> {
    general.send_message(&project_id, &text).map_err(refused)
}

/// The cards of questions of the project's Chat, in publication order: fetched when the view
/// loads and after each `chat-cards-changed`.
#[tauri::command]
pub async fn general_cards(
    general: State<'_, Arc<General>>,
    project_id: String,
) -> Result<ChatCards, AppError> {
    general.cards(&project_id).map_err(refused)
}

/// What the developer does in the active card of questions; the cards afterwards.
#[tauri::command]
pub async fn general_card_act(
    general: State<'_, Arc<General>>,
    project_id: String,
    card: String,
    action: CardActionInput,
) -> Result<ChatCards, AppError> {
    general
        .card_act(&project_id, &card, action.into())
        .map_err(refused)
}

/// `developer.SEND_ANSWERS`: the answers of the active card go to the agent as a message of the
/// developer, only when every question is answered or skipped.
#[tauri::command]
pub async fn general_send_answers(
    general: State<'_, Arc<General>>,
    project_id: String,
    card: String,
) -> Result<(), AppError> {
    general.send_answers(&project_id, &card).map_err(refused)
}

/// `developer.STOP_AGENT`.
#[tauri::command]
pub async fn general_stop(
    general: State<'_, Arc<General>>,
    project_id: String,
) -> Result<(), AppError> {
    general.stop(&project_id).map_err(refused)
}

/// `developer.ANSWER_PERMISSION`.
#[tauri::command]
pub async fn general_answer_permission(
    general: State<'_, Arc<General>>,
    project_id: String,
    request: u32,
    option: String,
) -> Result<(), AppError> {
    general
        .answer_permission(&project_id, u64::from(request), &option)
        .map_err(refused)
}

/// `developer.CLEAR_GENERAL`, once confirmed.
#[tauri::command]
pub async fn general_clear(
    general: State<'_, Arc<General>>,
    project_id: String,
) -> Result<(), AppError> {
    general.clear(&project_id).map_err(refused)
}

/// The list of the sub-agents of the project's General.
#[tauri::command]
pub async fn general_sub_agents(
    general: State<'_, Arc<General>>,
    project_id: String,
) -> Result<SubAgentsEvent, AppError> {
    general.sub_agents(&project_id).map_err(refused)
}

/// The read-only conversation of the sub-agent at `path` of the project's General.
#[tauri::command]
pub async fn general_sub_agent_conversation(
    general: State<'_, Arc<General>>,
    project_id: String,
    path: Vec<Step>,
) -> Result<SubAgentConversation, AppError> {
    general
        .sub_agent_conversation(&project_id, &path)
        .map_err(refused)
}

/// What the Chat view shows of the sub-agents: the conversation of which one, if any. The Chat has
/// no list of them on screen, and a transcript is read only while it is shown.
#[tauri::command]
pub async fn general_sub_agents_view(
    general: State<'_, Arc<General>>,
    project_id: String,
    path: Option<Vec<Step>>,
) -> Result<(), AppError> {
    general.sub_agents_view(&project_id, path).map_err(refused)
}
