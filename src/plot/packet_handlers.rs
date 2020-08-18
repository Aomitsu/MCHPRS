use super::Plot;
use crate::blocks::{BlockFace, BlockPos};
use crate::items::{Item, ItemStack, UseOnBlockContext};
use crate::network::packets::clientbound::*;
use crate::network::packets::serverbound::*;
use crate::network::packets::SlotData;
use crate::player::SkinParts;
use crate::server::Message;
use crate::world::World;
use log::debug;
use std::time::Instant;

impl Plot {
    pub(super) fn handle_packets_for_player(&mut self, player: usize) {
        let packets = self.players[player].client.receive_packets();
        for packet in packets {
            packet.handle(self, player);
        }
    }
}
impl ServerBoundPacketHandler for Plot {
    fn handle_keep_alive(&mut self, _keep_alive: S10KeepAlive, player_idx: usize) {
        self.players[player_idx].last_keep_alive_received = Instant::now();
    }

    fn handle_creative_inventory_action(
        &mut self,
        creative_inventory_action: S27CreativeInventoryAction,
        player: usize,
    ) {
        if let Some(slot_data) = creative_inventory_action.clicked_item {
            if creative_inventory_action.slot < 0 || creative_inventory_action.slot >= 46 {
                return;
            }
            let item = ItemStack {
                count: slot_data.item_count as u8,
                damage: 0,
                item_type: Item::from_id(slot_data.item_id as u32),
                nbt: slot_data.nbt,
            };
            self.players[player].inventory[creative_inventory_action.slot as usize] = Some(item);
            if creative_inventory_action.slot as u32 == self.players[player].selected_slot + 36 {
                let entity_equipment = C47EntityEquipment {
                    entity_id: self.players[player].entity_id as i32,
                    equipment: vec![C47EntityEquipmentEquipment {
                        slot: 0, // Main hand
                        item: self.players[player].inventory
                            [creative_inventory_action.slot as usize]
                            .as_ref()
                            .map(|item| SlotData {
                                item_count: item.count as i8,
                                item_id: item.item_type.get_id() as i32,
                                nbt: item.nbt.clone(),
                            }),
                    }],
                }
                .encode();
                for other_player in 0..self.players.len() {
                    if player == other_player {
                        continue;
                    };
                    self.players[other_player]
                        .client
                        .send_packet(&entity_equipment);
                }
            }
        } else {
            self.players[player].inventory[creative_inventory_action.slot as usize] = None;
        }
    }

    fn handle_player_abilities(&mut self, player_abilities: S1APlayerAbilities, player: usize) {
        self.players[player].flying = player_abilities.is_flying;
    }

    fn handle_animation(&mut self, animation: S2BAnimation, player: usize) {
        let animation_id = match animation.hand {
            0 => 0,
            1 => 3,
            _ => 0,
        };
        let entity_animation = C05EntityAnimation {
            entity_id: self.players[player].entity_id as i32,
            animation: animation_id,
        }
        .encode();
        for other_player in 0..self.players.len() {
            if player == other_player {
                continue;
            };
            self.players[other_player]
                .client
                .send_packet(&entity_animation);
        }
    }

    fn handle_player_block_placement(
        &mut self,
        player_block_placement: S2DPlayerBlockPlacemnt,
        player: usize,
    ) {
        let block_face = BlockFace::from_id(player_block_placement.face as u32);

        let selected_slot = self.players[player].selected_slot as usize;
        let item_in_hand = if player_block_placement.hand == 0 {
            self.players[player].inventory[selected_slot + 36].clone()
        } else {
            self.players[player].inventory[45].clone()
        };

        let block_pos = BlockPos::new(
            player_block_placement.x,
            player_block_placement.y as u32,
            player_block_placement.z,
        );

        if !Plot::in_plot_bounds(self.x, self.z, block_pos.x, block_pos.z) {
            self.players[player].send_system_message("Can't interact with blocks outside of plot");
            self.send_block_change(block_pos.offset(block_face), 0);
            return;
        }

        if let Some(item) = item_in_hand {
            item.use_on_block(
                self,
                UseOnBlockContext {
                    block_face,
                    block_pos,
                    player_crouching: self.players[player].crouching,
                    player_direction: self.players[player].get_direction(),
                    player_idx: player,
                },
            );
        } else {
            let block = self.get_block(block_pos);
            if !self.players[player].crouching {
                block.on_use(self, block_pos, None);
            }
        }
    }

    // Returns true if packets should stop being handled
    fn handle_chat_message(&mut self, chat_message: S03ChatMessage, player: usize) {
        let message = chat_message.message;
        if message.starts_with('/') {
            self.players[player].command_queue.push(message);
        } else {
            let player = &self.players[player];
            let broadcast_message =
                Message::ChatInfo(player.uuid, player.username.to_owned(), message);
            self.message_sender.send(broadcast_message).unwrap();
        }
    }

    fn handle_client_settings(&mut self, client_settings: S05ClientSettings, player: usize) {
        let player = &mut self.players[player];
        player.skin_parts =
            SkinParts::from_bits_truncate(client_settings.displayed_skin_parts as u32);
        let metadata_entry = C44EntityMetadataEntry {
            index: 16,
            metadata_type: 0,
            value: vec![player.skin_parts.bits() as u8],
        };
        let entity_metadata = C44EntityMetadata {
            entity_id: player.entity_id as i32,
            metadata: vec![metadata_entry],
        }
        .encode();
        for player in &mut self.players {
            player.client.send_packet(&entity_metadata);
        }
    }

    fn handle_plugin_message(&mut self, plugin_message: S0BPluginMessage, _player: usize) {
        debug!(
            "Client initiated plugin channel: {:?}",
            plugin_message.channel
        );
    }

    fn handle_player_position(&mut self, player_position: S12PlayerPosition, player: usize) {
        let old_x = self.players[player].x;
        let old_y = self.players[player].y;
        let old_z = self.players[player].z;
        let new_x = player_position.x;
        let new_y = player_position.y;
        let new_z = player_position.z;
        self.players[player].x = player_position.x;
        self.players[player].y = player_position.y;
        self.players[player].z = player_position.z;
        self.players[player].on_ground = player_position.on_ground;
        let packet = if (new_x - old_x).abs() > 8.0
            || (new_y - old_y).abs() > 8.0
            || (new_z - old_z).abs() > 8.0
        {
            C56EntityTeleport {
                entity_id: self.players[player].entity_id as i32,
                x: new_x,
                y: new_y,
                z: new_z,
                yaw: self.players[player].yaw,
                pitch: self.players[player].pitch,
                on_ground: player_position.on_ground,
            }
            .encode()
        } else {
            let delta_x = ((player_position.x * 32.0 - old_x * 32.0) * 128.0) as i16;
            let delta_y = ((player_position.y * 32.0 - old_y * 32.0) * 128.0) as i16;
            let delta_z = ((player_position.z * 32.0 - old_z * 32.0) * 128.0) as i16;
            C28EntityPosition {
                delta_x,
                delta_y,
                delta_z,
                entity_id: self.players[player].entity_id as i32,
                on_ground: player_position.on_ground,
            }
            .encode()
        };
        for other_player in 0..self.players.len() {
            if player == other_player {
                continue;
            };
            self.players[other_player].client.send_packet(&packet);
        }
    }

    fn handle_player_position_and_rotation(
        &mut self,
        player_position_and_rotation: S13PlayerPositionAndRotation,
        player: usize,
    ) {
        // This is beautiful
        let old_x = self.players[player].x;
        let old_y = self.players[player].y;
        let old_z = self.players[player].z;
        let new_x = player_position_and_rotation.x;
        let new_y = player_position_and_rotation.y;
        let new_z = player_position_and_rotation.z;
        self.players[player].x = player_position_and_rotation.x;
        self.players[player].y = player_position_and_rotation.y;
        self.players[player].z = player_position_and_rotation.z;
        self.players[player].yaw = player_position_and_rotation.yaw;
        self.players[player].pitch = player_position_and_rotation.pitch;
        self.players[player].on_ground = player_position_and_rotation.on_ground;
        let packet = if (new_x - old_x).abs() > 8.0
            || (new_y - old_y).abs() > 8.0
            || (new_z - old_z).abs() > 8.0
        {
            C56EntityTeleport {
                entity_id: self.players[player].entity_id as i32,
                x: new_x,
                y: new_y,
                z: new_z,
                yaw: self.players[player].yaw,
                pitch: self.players[player].pitch,
                on_ground: player_position_and_rotation.on_ground,
            }
            .encode()
        } else {
            let delta_x = ((player_position_and_rotation.x * 32.0 - old_x * 32.0) * 128.0) as i16;
            let delta_y = ((player_position_and_rotation.y * 32.0 - old_y * 32.0) * 128.0) as i16;
            let delta_z = ((player_position_and_rotation.z * 32.0 - old_z * 32.0) * 128.0) as i16;
            C29EntityPositionAndRotation {
                delta_x,
                delta_y,
                delta_z,
                pitch: player_position_and_rotation.pitch,
                yaw: player_position_and_rotation.yaw,
                entity_id: self.players[player].entity_id as i32,
                on_ground: player_position_and_rotation.on_ground,
            }
            .encode()
        };
        let entity_head_look = C3BEntityHeadLook {
            entity_id: self.players[player].entity_id as i32,
            yaw: player_position_and_rotation.yaw,
        }
        .encode();
        for other_player in 0..self.players.len() {
            if player == other_player {
                continue;
            };
            self.players[other_player].client.send_packet(&packet);
            self.players[other_player]
                .client
                .send_packet(&entity_head_look);
        }
    }

    fn handle_player_rotation(&mut self, player_rotation: S14PlayerRotation, player: usize) {
        self.players[player].yaw = player_rotation.yaw;
        self.players[player].pitch = player_rotation.pitch;
        self.players[player].on_ground = player_rotation.on_ground;
        let rotation_packet = C2AEntityRotation {
            entity_id: self.players[player].entity_id as i32,
            yaw: player_rotation.yaw,
            pitch: player_rotation.pitch,
            on_ground: player_rotation.on_ground,
        }
        .encode();
        let entity_head_look = C3BEntityHeadLook {
            entity_id: self.players[player].entity_id as i32,
            yaw: player_rotation.yaw,
        }
        .encode();
        for other_player in 0..self.players.len() {
            if player == other_player {
                continue;
            };
            self.players[other_player]
                .client
                .send_packet(&rotation_packet);
            self.players[other_player]
                .client
                .send_packet(&entity_head_look);
        }
    }

    fn handle_player_movement(&mut self, player_movement: S15PlayerMovement, player: usize) {
        self.players[player].on_ground = player_movement.on_ground;
        let packet = C2BEntityMovement {
            entity_id: self.players[player].entity_id as i32,
        }
        .encode();
        for other_player in 0..self.players.len() {
            if player == other_player {
                continue;
            };
            self.players[other_player].client.send_packet(&packet);
        }
    }

    fn handle_player_digging(&mut self, player_digging: S1BPlayerDigging, player: usize) {
        if player_digging.status == 0 {
            let block_pos =
                BlockPos::new(player_digging.x, player_digging.y as u32, player_digging.z);

            if !Plot::in_plot_bounds(self.x, self.z, block_pos.x, block_pos.z) {
                self.players[player].send_system_message("Can't break blocks outside of plot");
                return;
            }

            // This worldedit wand stuff should probably be done in another file. It's good enough for now.
            let item_in_hand = self.players[player].inventory
                [self.players[player].selected_slot as usize + 36]
                .clone();
            if let Some(item) = item_in_hand {
                if item.item_type == Item::WEWand {
                    let block = self.get_block(block_pos);
                    self.send_block_change(block_pos, block.get_id());
                    if let Some(pos) = self.players[player].first_position {
                        if pos == block_pos {
                            return;
                        }
                    }
                    self.players[player].worldedit_set_first_position(
                        block_pos.x,
                        block_pos.y,
                        block_pos.z,
                    );
                    return;
                }
            }

            let other_block = self.get_block(block_pos);
            other_block.destroy(self, block_pos);

            let effect = C22Effect {
                effect_id: 2001,
                x: player_digging.x,
                y: player_digging.y,
                z: player_digging.z,
                data: other_block.get_id() as i32,
                disable_relative_volume: false,
            }
            .encode();
            for other_player in 0..self.players.len() {
                if player == other_player {
                    continue;
                };
                self.players[other_player].client.send_packet(&effect);
            }
        } else {
            let selected_slot = self.players[player].selected_slot as usize + 36;
            if player_digging.status == 3 {
                self.players[player].inventory[selected_slot] = None;
            } else if player_digging.status == 4 {
                let mut stack_empty = false;
                if let Some(item_stack) = &mut self.players[player].inventory[selected_slot] {
                    item_stack.count -= 1;
                    stack_empty = item_stack.count == 0;
                }
                if stack_empty {
                    self.players[player].inventory[selected_slot] = None;
                }
            }
        }
    }

    fn handle_entity_action(&mut self, entity_action: S1CEntityAction, player: usize) {
        match entity_action.action_id {
            0 => self.players[player].crouching = true,
            1 => self.players[player].crouching = false,
            3 => self.players[player].sprinting = true,
            4 => self.players[player].sprinting = false,
            _ => {}
        }
        let mut bitfield = 0;
        if self.players[player].crouching {
            bitfield |= 0x02
        };
        if self.players[player].sprinting {
            bitfield |= 0x08
        };
        let mut metadata_entries = Vec::new();
        metadata_entries.push(C44EntityMetadataEntry {
            index: 0,
            metadata_type: 0,
            value: vec![bitfield],
        });
        metadata_entries.push(C44EntityMetadataEntry {
            index: 6,
            metadata_type: 18,
            value: vec![if self.players[player].crouching { 5 } else { 0 }],
        });
        let entity_metadata = C44EntityMetadata {
            entity_id: self.players[player].entity_id as i32,
            metadata: metadata_entries,
        }
        .encode();
        for other_player in 0..self.players.len() {
            if player == other_player {
                continue;
            };
            self.players[other_player]
                .client
                .send_packet(&entity_metadata);
        }
    }

    fn handle_held_item_change(&mut self, held_item_change: S24HeldItemChange, player: usize) {
        let entity_equipment = C47EntityEquipment {
            entity_id: self.players[player].entity_id as i32,
            equipment: vec![C47EntityEquipmentEquipment {
                slot: 0, // Main hand
                item: self.players[player].inventory[held_item_change.slot as usize + 36]
                    .as_ref()
                    .map(|item| SlotData {
                        item_count: item.count as i8,
                        item_id: item.item_type.get_id() as i32,
                        nbt: item.nbt.clone(),
                    }),
            }],
        }
        .encode();
        for other_player in 0..self.players.len() {
            if player == other_player {
                continue;
            };
            self.players[other_player]
                .client
                .send_packet(&entity_equipment);
        }
        self.players[player].selected_slot = held_item_change.slot as u32;
    }
}
