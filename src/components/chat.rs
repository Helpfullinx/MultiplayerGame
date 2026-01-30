use crate::components::player::PlayerInfo;
use crate::network::net_manage::TcpConnection;
use crate::network::net_message::{NetworkMessage, CTcpType, STcpType};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::{Changed, Component, KeyCode, Local, MessageReader, Query, Res, ResMut, Single, Text, With};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use crate::components::common::Id;

const CHAT_HISTORY_LEN: usize = 10;
const MAX_CHAT_MESSAGE_LENGTH: usize = 50;

#[derive(Component)]
pub struct Chat {
    pub chat_history: VecDeque<(Id, ChatMessage)>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub message: String,
}

pub fn chat_window(
    player_info: Res<PlayerInfo>,
    mut connection: Single<&mut TcpConnection<CTcpType>>,
    mut keyboard_input: MessageReader<KeyboardInput>,
    mut message_buffer: Local<String>,
    mut is_active: Local<bool>,
    mut chat: Query<(&mut Text, &mut Chat), With<Chat>>,
) {
    let message_full = message_buffer.len() >= MAX_CHAT_MESSAGE_LENGTH;
    
    for k in keyboard_input.read() {
        if k.state == ButtonState::Released {
            continue;
        }

        if *is_active {
            match &k.logical_key {
                Key::Backspace => {
                    message_buffer.pop();
                }
                Key::Enter => {
                    if connection.stream.is_some() {
                        connection.add_message(
                            NetworkMessage(CTcpType::ChatMessage {
                                player_id: player_info.current_player_id,
                                message: ChatMessage {
                                    message: message_buffer.clone(),
                                },
                        }))
                    }
                    message_buffer.clear();
                    *is_active = false;
                }
                Key::Character(c) => {
                    if !message_full { message_buffer.push_str(c.as_str()) }
                }
                Key::Space => {
                    if !message_full { message_buffer.push_str(" ") }
                }
                _ => {}
            }
        }

        match k.key_code {
            KeyCode::KeyT => {
                *is_active = true;
            }
            KeyCode::Escape => {
                *is_active = false;
            }
            _ => {}
        }

        // println!("{:?}", message_buffer);
    }

    // Updates chat window
    if let Some(mut chat) = chat.single_mut().ok() {
        chat.0.0.clear();
        for c in chat.1.chat_history.iter_mut() {
            chat.0.0.push_str(&format!(
                "{:?}: {:?}\n",
                c.0.0.to_string(),
                c.1.message.to_string()
            ));
        }
        if *is_active {
            chat.0
                .0
                .push_str(&format!("{:?}\n", message_buffer.to_string()));
        }
    }
}

pub fn client_add_chat_message(
    messages: &mut Vec<(Id, ChatMessage)>,
    chat: &mut Query<&mut Chat>
) {
    if let Some(mut chat) = chat.single_mut().ok() {
        chat.chat_history.clear();
        while !messages.is_empty() {
            if chat.chat_history.len() >= CHAT_HISTORY_LEN {
                chat.chat_history.pop_back();
            }
            let message = messages.pop().unwrap();
            chat.chat_history.push_front(message);
        }
    }
}

pub fn server_add_chat_message(message: (Id, ChatMessage), chat: &mut Query<&mut Chat>) {
    if let Some(mut chat) = chat.single_mut().ok() {
        while chat.chat_history.len() >= CHAT_HISTORY_LEN {
            chat.chat_history.pop_front();
        }
        if !(message.1.message.len() > MAX_CHAT_MESSAGE_LENGTH) {
            chat.chat_history.push_back(message);
        }
    }
}

pub fn send_chat_to_all_connections(
    chat: Query<&mut Chat, Changed<Chat>>,
    mut connections: Query<&mut TcpConnection<STcpType>>,
) {
    if let Some(chat) = chat.single().ok() {
        for mut c in connections.iter_mut() {
            c.add_message(NetworkMessage(STcpType::Chat {
                messages: Vec::from(chat.chat_history.clone()),
            }));
        }
    }
}