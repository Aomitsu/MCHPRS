use super::{database, Plot};
use crate::server::Message;
use log::info;

use std::time::{Instant, Duration};

impl Plot {
    fn handle_plot_command(&mut self, player: usize, command: &str, args: Vec<&str>) {
        let plot_x = self.players[player].x as i32 >> 7;
        let plot_z = self.players[player].z as i32 >> 7;
        match command {
            "claim" | "c" => {
                if database::get_plot_owner(plot_x, plot_z).is_some() {
                    self.players[player].send_system_message("Plot is already claimed!");
                } else {
                    let uuid = format!("{}", self.players[player].uuid);
                    database::claim_plot(plot_x, plot_z, &uuid);
                    self.players[player]
                        .send_system_message(&format!("Claimed plot {},{}", plot_x, plot_z));
                }
            }
            "info" | "i" => {
                if let Some(owner) = database::get_plot_owner(plot_x, plot_z) {
                    self.players[player]
                        .send_system_message(&format!("Plot owner is: {:032x}", owner));
                } else {
                    self.players[player].send_system_message("Plot is not owned by anyone.");
                }
            }
            _ => self.players[player].send_system_message("Wrong argument for /plot"),
        }
    }

    pub(super) fn handle_command(&mut self, player: usize, command: &str, mut args: Vec<&str>) {
        info!(
            "{} issued command: {} {}",
            self.players[player].username,
            command,
            args.join(" ")
        );
        match command {
            "//1" | "//pos1" => {
                let player = &mut self.players[player];

                let mut x = player.x as i32;
                let mut y = player.y as u32;
                let mut z = player.z as i32;

                if args.len() >= 3 {
                    if let Ok(x_arg) = args[0].parse::<i32>() {
                        x = x_arg;
                    } else {
                        player.send_system_message("Unable to parse x coordinate!");
                        return;
                    }
                    if let Ok(y_arg) = args[1].parse::<u32>() {
                        y = y_arg;
                    } else {
                        player.send_system_message("Unable to parse y coordinate!");
                        return;
                    }
                    if let Ok(z_arg) = args[2].parse::<i32>() {
                        z = z_arg;
                    } else {
                        player.send_system_message("Unable to parse z coordinate!");
                        return;
                    }
                }

                player.worldedit_set_first_position(x, y, z);
            }
            "//2" | "//pos2" => {
                let player = &mut self.players[player];

                let mut x = player.x as i32;
                let mut y = player.y as u32;
                let mut z = player.z as i32;

                if args.len() >= 3 {
                    if let Ok(x_arg) = args[0].parse::<i32>() {
                        x = x_arg;
                    } else {
                        player.send_system_message("Unable to parse x coordinate!");
                        return;
                    }
                    if let Ok(y_arg) = args[1].parse::<u32>() {
                        y = y_arg;
                    } else {
                        player.send_system_message("Unable to parse y coordinate!");
                        return;
                    }
                    if let Ok(z_arg) = args[2].parse::<i32>() {
                        z = z_arg;
                    } else {
                        player.send_system_message("Unable to parse z coordinate!");
                        return;
                    }
                }

                player.worldedit_set_second_position(x, y, z);
            }
            "//set" => {
                if self.worldedit_set(player, &args[0]).is_err() {
                    self.players[player].send_system_message(
                        "Invalid block. Note that not all blocks are supported.",
                    );
                }
            }
            "//replace" => {
                if self.worldedit_replace(player, &args[0], &args[1]).is_err() {
                    self.players[player].send_system_message(
                        "Invalid block. Note that not all blocks are supported.",
                    );
                }
            }
            "//copy" | "//c" => self.worldedit_copy(player),
            "//paste" | "//p" => self.worldedit_paste(player),
            "//count" => {
                if self.worldedit_count(player, &args[0]).is_err() {
                    self.players[player].send_system_message(
                        "Invalid block. Note that not all blocks are supported.",
                    );
                }
            }
            "//load" => self.worldedit_load(player, &args[0]),
            "/rtps" => {
                if args.is_empty() {
                    self.players[player]
                        .send_system_message("Please specify the rtps you want to set to.");
                    return;
                }
                let tps = if let Ok(tps) = args[0].parse::<u32>() {
                    tps
                } else {
                    self.players[player].send_system_message("Unable to parse rtps!");
                    return;
                };
                if tps > 35000 {
                    self.players[player]
                        .send_system_message("The rtps cannot go higher than 35000!");
                    return;
                }
                self.lag_time = Duration::from_millis(0);
                if tps > 0 {
                    self.sleep_time = Duration::from_micros(1_000_000 / tps as u64);
                } else {
                    self.sleep_time = Duration::from_millis(2);
                }
                self.tps = tps;
                self.players[player].send_system_message("The rtps was successfully set.");
            }
            "/radv" | "/radvance" => {
                if args.is_empty() {
                    self.players[player]
                        .send_system_message("Please specify a number of ticks to advance.");
                    return;
                }
                let ticks = if let Ok(ticks) = args[0].parse::<u32>() {
                    ticks
                } else {
                    self.players[player].send_system_message("Unable to parse ticks!");
                    return;
                };
                let start_time = Instant::now();
                for _ in 0..ticks {
                    self.tick();
                }
                self.players[player].send_system_message(&format!("Plot has been advanced by {} ticks ({:?})", ticks, start_time.elapsed()));
            }
            "/teleport" | "/tp" => {
                if args.len() == 3 {
                    let x;
                    let y;
                    let z;
                    if let Ok(x_arg) = args[0].parse::<f64>() {
                        x = x_arg;
                    } else {
                        self.players[player].send_system_message("Unable to parse x coordinate!");
                        return;
                    }
                    if let Ok(y_arg) = args[1].parse::<f64>() {
                        y = y_arg;
                    } else {
                        self.players[player].send_system_message("Unable to parse y coordinate!");
                        return;
                    }
                    if let Ok(z_arg) = args[2].parse::<f64>() {
                        z = z_arg;
                    } else {
                        self.players[player].send_system_message("Unable to parse z coordinate!");
                        return;
                    }
                    self.players[player].teleport(x, y, z);
                } else if args.len() == 1 {
                    let player = self.leave_plot(player);
                    self.message_sender
                        .send(Message::PlayerTeleportOther(player, args[0].to_string()));
                } else {
                    self.players[player]
                        .send_system_message("Wrong number of arguments for teleport command!");
                }
            }
            "/stop" => {
                self.message_sender.send(Message::Shutdown);
            }
            "/plot" | "/p" => {
                if args.is_empty() {
                    self.players[player].send_system_message("Wrong number of arguments!");
                    return;
                }
                let command = args.remove(0);
                self.handle_plot_command(player, command, args);
            }
            _ => self.players[player].send_system_message("Command not found!"),
        }
    }
}
