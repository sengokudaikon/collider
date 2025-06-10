use anyhow::{Result, anyhow};
use dialoguer::{Confirm, Input};

use crate::cli::{Commands, ProgressMode};

pub struct SeederConfig {
    pub min_users: usize,
    pub max_users: usize,
    pub min_event_types: usize,
    pub max_event_types: usize,
    pub target_events: usize,
    pub event_batch_size: Option<usize>,
}

impl SeederConfig {
    pub fn from_commands(
        commands: Commands, mode: ProgressMode,
    ) -> Result<Commands> {
        match commands {
            Commands::All {
                min_users,
                max_users,
                min_event_types,
                max_event_types,
                target_events,
                event_batch_size,
            } => {
                let config = match mode {
                    ProgressMode::Quiet => {
                        // Require all arguments in quiet mode
                        SeederConfig {
                            min_users: min_users.ok_or_else(|| {
                                anyhow!(
                                    "--min-users is required in quiet mode"
                                )
                            })?,
                            max_users: max_users.ok_or_else(|| {
                                anyhow!(
                                    "--max-users is required in quiet mode"
                                )
                            })?,
                            min_event_types: min_event_types.ok_or_else(
                                || {
                                    anyhow!(
                                        "--min-event-types is required in \
                                         quiet mode"
                                    )
                                },
                            )?,
                            max_event_types: max_event_types.ok_or_else(
                                || {
                                    anyhow!(
                                        "--max-event-types is required in \
                                         quiet mode"
                                    )
                                },
                            )?,
                            target_events: target_events.ok_or_else(
                                || {
                                    anyhow!(
                                        "--target-events is required in \
                                         quiet mode"
                                    )
                                },
                            )?,
                            event_batch_size,
                        }
                    }
                    ProgressMode::Interactive => {
                        // Prompt for missing values
                        let min_users = min_users.unwrap_or_else(|| {
                            Input::new()
                                .with_prompt(
                                    "Minimum number of users to generate",
                                )
                                .default(10_000)
                                .interact()
                                .unwrap_or(10_000)
                        });

                        let max_users = max_users.unwrap_or_else(|| {
                            Input::new()
                                .with_prompt(
                                    "Maximum number of users to generate",
                                )
                                .default(100_000)
                                .interact()
                                .unwrap_or(100_000)
                        });

                        let min_event_types =
                            min_event_types.unwrap_or_else(|| {
                                Input::new()
                                    .with_prompt(
                                        "Minimum number of event types to \
                                         generate",
                                    )
                                    .default(50)
                                    .interact()
                                    .unwrap_or(50)
                            });

                        let max_event_types =
                            max_event_types.unwrap_or_else(|| {
                                Input::new()
                                    .with_prompt(
                                        "Maximum number of event types to \
                                         generate",
                                    )
                                    .default(200)
                                    .interact()
                                    .unwrap_or(200)
                            });

                        let target_events =
                            target_events.unwrap_or_else(|| {
                                Input::new()
                                    .with_prompt(
                                        "Total number of events to generate",
                                    )
                                    .default(1_000_000)
                                    .interact()
                                    .unwrap_or(1_000_000)
                            });

                        let event_batch_size = if event_batch_size.is_none() {
                            if Confirm::new()
                                .with_prompt(
                                    "Use custom batch size for events?",
                                )
                                .default(false)
                                .interact()
                                .unwrap_or(false)
                            {
                                Some(
                                    Input::new()
                                        .with_prompt("Event batch size")
                                        .default(10_000)
                                        .interact()
                                        .unwrap_or(10_000),
                                )
                            }
                            else {
                                None
                            }
                        }
                        else {
                            event_batch_size
                        };

                        SeederConfig {
                            min_users,
                            max_users,
                            min_event_types,
                            max_event_types,
                            target_events,
                            event_batch_size,
                        }
                    }
                };

                // Validate ranges
                if config.min_users > config.max_users {
                    return Err(anyhow!(
                        "min-users ({}) cannot be greater than max-users \
                         ({})",
                        config.min_users,
                        config.max_users
                    ));
                }
                if config.min_event_types > config.max_event_types {
                    return Err(anyhow!(
                        "min-event-types ({}) cannot be greater than \
                         max-event-types ({})",
                        config.min_event_types,
                        config.max_event_types
                    ));
                }

                Ok(Commands::All {
                    min_users: Some(config.min_users),
                    max_users: Some(config.max_users),
                    min_event_types: Some(config.min_event_types),
                    max_event_types: Some(config.max_event_types),
                    target_events: Some(config.target_events),
                    event_batch_size: config.event_batch_size,
                })
            }
            Commands::Users {
                min_users,
                max_users,
            } => {
                let (min_users, max_users) = match mode {
                    ProgressMode::Quiet => {
                        (
                            min_users.ok_or_else(|| {
                                anyhow!(
                                    "--min-users is required in quiet mode"
                                )
                            })?,
                            max_users.ok_or_else(|| {
                                anyhow!(
                                    "--max-users is required in quiet mode"
                                )
                            })?,
                        )
                    }
                    ProgressMode::Interactive => {
                        let min_users = min_users.unwrap_or_else(|| {
                            Input::new()
                                .with_prompt(
                                    "Minimum number of users to generate",
                                )
                                .default(10_000)
                                .interact()
                                .unwrap_or(10_000)
                        });

                        let max_users = max_users.unwrap_or_else(|| {
                            Input::new()
                                .with_prompt(
                                    "Maximum number of users to generate",
                                )
                                .default(100_000)
                                .interact()
                                .unwrap_or(100_000)
                        });

                        (min_users, max_users)
                    }
                };

                if min_users > max_users {
                    return Err(anyhow!(
                        "min-users ({}) cannot be greater than max-users \
                         ({})",
                        min_users,
                        max_users
                    ));
                }

                Ok(Commands::Users {
                    min_users: Some(min_users),
                    max_users: Some(max_users),
                })
            }
            Commands::EventTypes {
                min_types,
                max_types,
            } => {
                let (min_types, max_types) = match mode {
                    ProgressMode::Quiet => {
                        (
                            min_types.ok_or_else(|| {
                                anyhow!(
                                    "--min-types is required in quiet mode"
                                )
                            })?,
                            max_types.ok_or_else(|| {
                                anyhow!(
                                    "--max-types is required in quiet mode"
                                )
                            })?,
                        )
                    }
                    ProgressMode::Interactive => {
                        let min_types = min_types.unwrap_or_else(|| {
                            Input::new()
                                .with_prompt(
                                    "Minimum number of event types to \
                                     generate",
                                )
                                .default(50)
                                .interact()
                                .unwrap_or(50)
                        });

                        let max_types = max_types.unwrap_or_else(|| {
                            Input::new()
                                .with_prompt(
                                    "Maximum number of event types to \
                                     generate",
                                )
                                .default(200)
                                .interact()
                                .unwrap_or(200)
                        });

                        (min_types, max_types)
                    }
                };

                if min_types > max_types {
                    return Err(anyhow!(
                        "min-types ({}) cannot be greater than max-types \
                         ({})",
                        min_types,
                        max_types
                    ));
                }

                Ok(Commands::EventTypes {
                    min_types: Some(min_types),
                    max_types: Some(max_types),
                })
            }
            Commands::Events {
                target_events,
                batch_size,
            } => {
                let (target_events, batch_size) = match mode {
                    ProgressMode::Quiet => {
                        (
                            target_events.ok_or_else(|| {
                                anyhow!(
                                    "--target-events is required in quiet \
                                     mode"
                                )
                            })?,
                            batch_size,
                        )
                    }
                    ProgressMode::Interactive => {
                        let target_events =
                            target_events.unwrap_or_else(|| {
                                Input::new()
                                    .with_prompt(
                                        "Total number of events to generate",
                                    )
                                    .default(1_000_000)
                                    .interact()
                                    .unwrap_or(1_000_000)
                            });

                        let batch_size = if batch_size.is_none() {
                            if Confirm::new()
                                .with_prompt("Use custom batch size?")
                                .default(false)
                                .interact()
                                .unwrap_or(false)
                            {
                                Some(
                                    Input::new()
                                        .with_prompt("Event batch size")
                                        .default(10_000)
                                        .interact()
                                        .unwrap_or(10_000),
                                )
                            }
                            else {
                                None
                            }
                        }
                        else {
                            batch_size
                        };

                        (target_events, batch_size)
                    }
                };

                Ok(Commands::Events {
                    target_events: Some(target_events),
                    batch_size,
                })
            }
        }
    }
}
