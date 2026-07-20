#[cfg(feature = "server")]
mod auth;
mod backend;
mod components;
mod gamification;
mod models;

use async_std::task::sleep;
use backend::{
    get_leaderboard, get_private_profile, get_recent_leaderboard_days, get_story, save_typing_result,
};
use components::{
    avatar::{AvatarImageSize, ImageAvatar},
    button::{Button, ButtonSize, ButtonVariant},
};
use dioxus::prelude::*;
use gamification::{
    Badge, LocalStats, PaceStatus, award_badges, complete_daily_challenge, current_challenge_date,
    load_local_stats, pace_vs_best, record_combo_word, save_local_stats, update_personal_bests,
};
use jiff::Timestamp;
use models::{
    Leaderboard, LeaderboardScope, PrivateProfile, Story, TypingSubmission, calculate_typing_metrics,
};
use wasm_bindgen::prelude::*;

const DEFAULT_TITLE: &str = "";
const TEST_DURATION_SECONDS: i64 = 60;

const FAVICON: Asset = asset!("/assets/favicon.ico");
const APP_CSS: Asset = asset!("/assets/main.css");
const COMPONENTS_CSS: Asset = asset!("/assets/dx-components-theme.css");
const GITHUB_LOGO: Asset = asset!("/assets/github_logo.png");
const HEADER_MAIN: Asset = asset!("assets/logo_blazing_board.png");

fn main() {
    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        use dioxus::server::axum::routing::{get, post};

        Ok(dioxus::server::router(App)
            .route("/auth/github", get(auth::github_login))
            .route("/auth/github/callback", get(auth::github_callback))
            .route("/auth/logout", post(auth::github_logout)))
    });

    #[cfg(not(feature = "server"))]
    dioxus::launch(App);
}

#[wasm_bindgen]
pub fn get_timestamp_seconds_now_wasm() -> i64 {
    let now: Timestamp = Timestamp::now();
    now.as_second()
}

#[wasm_bindgen]
pub fn get_timestamp_milliseconds_now_wasm() -> i64 {
    let now: Timestamp = Timestamp::now();
    now.as_millisecond()
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: COMPONENTS_CSS }
        document::Link { rel: "stylesheet", href: APP_CSS }
        a {
            id: "gh",
            href: "https://github.com/wdroz/BlazingBoard",
            target: "_blank",
            img { src: GITHUB_LOGO }
            span { "repo" }
        }
        TypingWords {}
    }
}

#[component]
pub fn TypingWords() -> Element {
    let mut current_chunk_index = use_signal(|| 0);
    let mut current_word_in_chunk_index = use_signal(|| 0);
    let mut current_text = use_signal(String::new);
    let mut user_words = use_signal(Vec::<String>::new);
    let mut started_at = use_signal(|| None::<i64>);
    let mut run_id = use_signal(String::new);
    let mut correct_words = use_signal(|| 0_i64);
    let mut wrong_words = use_signal(|| 0_i64);
    let mut duration_seconds = use_signal(|| 0_i64);
    let mut timer_value = use_signal(|| TEST_DURATION_SECONDS);
    let mut running = use_signal(|| false);
    let mut finished = use_signal(|| false);
    let mut submitted_run = use_signal(|| None::<String>);
    let mut save_message = use_signal(String::new);
    let mut combo = use_signal(|| 0_i64);
    let mut max_combo = use_signal(|| 0_i64);
    let mut combo_milestone = use_signal(|| None::<i64>);
    let mut local_stats = use_signal(LocalStats::default);
    let mut local_stats_loaded = use_signal(|| false);
    let mut processed_gamification_run = use_signal(|| None::<String>);
    let mut new_badge = use_signal(|| None::<Badge>);
    let mut new_personal_best = use_signal(|| false);
    let challenge_date = use_signal(current_challenge_date);

    use_effect(move || {
        local_stats.set(load_local_stats());
        local_stats_loaded.set(true);
    });

    let re_story = use_resource(|| async move {
        get_story().await.unwrap_or(Story {
            ..Default::default()
        })
    });
    let profile_resource =
        use_resource(|| async move { get_private_profile().await.unwrap_or(None) });
    let mut leaderboard_scope = use_signal(|| LeaderboardScope::Day);
    let mut leaderboard_day = use_signal(current_challenge_date);
    let recent_days_resource = use_resource(|| async move {
        get_recent_leaderboard_days()
            .await
            .unwrap_or_else(|_| vec![current_challenge_date()])
    });
    let leaderboard_resource = use_resource(move || {
        let scope = leaderboard_scope();
        let day = leaderboard_day();
        async move {
            get_leaderboard(scope.as_str().to_string(), Some(day))
                .await
                .ok()
        }
    });

    let story = re_story().unwrap_or(Story {
        ..Default::default()
    });
    let profile = profile_resource().unwrap_or(None);
    let last_title = story
        .title
        .clone()
        .unwrap_or_else(|| DEFAULT_TITLE.to_string());
    let sentence_to_write_words = story
        .story
        .split_whitespace()
        .map(|w| w.to_string())
        .collect::<Vec<String>>();

    let sentence_to_write_chunks = sentence_to_write_words
        .chunks(15)
        .map(|chunk| chunk.to_vec())
        .collect::<Vec<Vec<String>>>();

    let nb_chunks_to_write = sentence_to_write_chunks.len();

    let _ = use_coroutine(move |_: UnboundedReceiver<i32>| async move {
        loop {
            sleep(std::time::Duration::from_secs(1)).await;
            if running() {
                if timer_value() <= 1 {
                    timer_value.set(0);
                    duration_seconds.set(TEST_DURATION_SECONDS);
                    running.set(false);
                    finished.set(true);
                } else {
                    timer_value.set(timer_value() - 1);
                }
            }
        }
    });

    use_effect(move || {
        let profile_is_loaded = profile_resource().unwrap_or(None).is_some();
        let current_run_id = run_id();
        let should_save = finished()
            && profile_is_loaded
            && !current_run_id.is_empty()
            && submitted_run().as_deref() != Some(current_run_id.as_str());

        if should_save {
            submitted_run.set(Some(current_run_id.clone()));
            save_message.set("Saving result…".to_string());
            let submission = TypingSubmission {
                run_id: current_run_id,
                story_when: story.when,
                correct_words: correct_words(),
                wrong_words: wrong_words(),
                duration_seconds: duration_seconds(),
            };
            let mut profile_resource = profile_resource;
            let mut leaderboard_resource = leaderboard_resource;

            spawn(async move {
                match save_typing_result(submission).await {
                    Ok(_) => {
                        save_message.set("Saved to your private history.".to_string());
                        profile_resource.restart();
                        leaderboard_resource.restart();
                    }
                    Err(_) => {
                        save_message.set("This result could not be saved.".to_string());
                    }
                }
            });
        }
    });

    use_effect(move || {
        let current_run_id = run_id();
        let should_process = finished()
            && local_stats_loaded()
            && !current_run_id.is_empty()
            && processed_gamification_run().as_deref() != Some(current_run_id.as_str());

        if !should_process {
            return;
        }

        processed_gamification_run.set(Some(current_run_id));
        let Ok(run_metrics) =
            calculate_typing_metrics(correct_words(), wrong_words(), duration_seconds().max(1))
        else {
            return;
        };
        let mut updated_stats = local_stats();
        complete_daily_challenge(&mut updated_stats, &challenge_date());
        new_personal_best.set(update_personal_bests(
            &mut updated_stats,
            run_metrics.wpm,
            run_metrics.accuracy,
            run_metrics.score,
        ));
        new_badge.set(award_badges(
            &mut updated_stats,
            run_metrics.accuracy,
            run_metrics.wpm,
        ));
        save_local_stats(&updated_stats);
        local_stats.set(updated_stats);
    });

    let current_chunk = {
        let index = current_chunk_index();
        if index < nb_chunks_to_write {
            sentence_to_write_chunks[index].clone()
        } else {
            vec![]
        }
    };
    let next_chunk = {
        let index = current_chunk_index();
        if index + 1 < nb_chunks_to_write {
            sentence_to_write_chunks[index + 1].clone()
        } else {
            vec![]
        }
    };

    let metrics =
        calculate_typing_metrics(correct_words(), wrong_words(), duration_seconds().max(1)).ok();
    let accuracy_percent = metrics
        .map(|current| current.accuracy * 100.0)
        .unwrap_or(0.0);
    let wpm = metrics.map(|current| current.wpm).unwrap_or(0.0);
    let score = metrics.map(|current| current.score).unwrap_or(0);
    let stats = local_stats();
    let completed_today = stats.last_completion_date.as_deref() == Some(challenge_date().as_str());
    let elapsed_seconds = if running() || finished() {
        (TEST_DURATION_SECONDS - timer_value()).clamp(0, TEST_DURATION_SECONDS)
    } else {
        0
    };
    let pace = if running() {
        pace_vs_best(correct_words(), elapsed_seconds, stats.best_wpm)
    } else {
        None
    };
    let combo_class = if combo_milestone().is_some() {
        "combo-display combo-milestone"
    } else if combo() > 0 {
        "combo-display combo-active"
    } else {
        "combo-display"
    };
    let timer_class = if running() && timer_value() <= 5 {
        "timer-urgent"
    } else {
        ""
    };
    let results_class = if new_personal_best() {
        "results-record"
    } else {
        ""
    };

    rsx! {
        div { id: "TypingWords",
            ProfileBar { profile: profile.clone() }
            section { class: "daily-status", aria_label: "Daily challenge status",
                div {
                    span { class: "daily-label", "Daily challenge" }
                    strong {
                        if completed_today {
                            "Complete for today"
                        } else {
                            "Ready to blaze"
                        }
                    }
                }
                div { class: "streak-count",
                    span { aria_hidden: "true", "🔥" }
                    strong { "{stats.streak}" }
                    span { " day streak" }
                }
            }
            if stats.best_score > 0 {
                p { class: "personal-best",
                    "Best {stats.best_wpm:.0} WPM · {stats.best_accuracy * 100.0:.0}% · {stats.best_score} pts"
                }
            }
            div { id: "TypingTitle", "{last_title}" }
            img { src: HEADER_MAIN, id: "brand-logo" }
            div { id: "timer", class: "{timer_class}", "{timer_value}" }
            div {
                class: "{combo_class}",
                span { class: "combo-flame", aria_hidden: "true", "🔥" }
                strong { "{combo}" }
                span { " word combo" }
                if let Some(milestone) = combo_milestone() {
                    em { role: "status", aria_live: "polite", "{milestone} word blaze!" }
                }
            }
            if let Some(pace_status) = pace {
                p {
                    class: match pace_status {
                        PaceStatus::Ahead => "pace-status pace-ahead",
                        PaceStatus::Behind => "pace-status pace-behind",
                        PaceStatus::Even => "pace-status pace-even",
                    },
                    role: "status",
                    match pace_status {
                        PaceStatus::Ahead => "Ahead of your best pace",
                        PaceStatus::Behind => "Behind your best pace",
                        PaceStatus::Even => "Matching your best pace",
                    }
                }
            }
            div { id: "words",
                for (i , word) in current_chunk.iter().enumerate() {
                    if i < current_word_in_chunk_index() {
                        if user_words().len() > i {
                            if user_words()[i] == *word {
                                div { class: "previous_correct", "{word}" }
                            } else {
                                div { class: "previous_wrong", "{word}" }
                            }
                        }
                    } else if i == current_word_in_chunk_index() {
                        div { id: "current", "{word}" }
                    } else {
                        div { "{word}" }
                    }
                }
                div { class: "break" }
                for word in next_chunk.iter() {
                    div { "{word}" }
                }
            }
            if !running() && !finished() {
                div { id: "tips",
                    "Write as quickly as you can, pressing the space bar after each word."
                }
            }
            if current_chunk_index() < nb_chunks_to_write && !finished() {
                input {
                    id: "textUser",
                    oninput: move |event| {
                        let current_chunk_clone = current_chunk.clone();
                        async move {
                            if started_at().is_none() {
                                started_at.set(Some(get_timestamp_seconds_now_wasm()));
                                run_id.set(format!(
                                    "run-{}",
                                    get_timestamp_milliseconds_now_wasm()
                                ));
                                running.set(true);
                            }
                            let data = event.value();
                            if data.ends_with(" ") {
                                let typed_word = data.trim().to_string();
                                let mut new_words = user_words().to_vec();
                                new_words.push(typed_word.clone());
                                user_words.set(new_words);

                                let word_index = current_word_in_chunk_index();
                                let is_correct = current_chunk_clone
                                    .get(word_index)
                                    .is_some_and(|expected| expected == &typed_word);
                                if is_correct {
                                    correct_words.set(correct_words() + 1);
                                } else {
                                    wrong_words.set(wrong_words() + 1);
                                }
                                let combo_progress =
                                    record_combo_word(combo(), max_combo(), is_correct);
                                combo.set(combo_progress.current);
                                max_combo.set(combo_progress.best);
                                combo_milestone.set(combo_progress.milestone);

                                let next_word_index = word_index + 1;
                                if next_word_index >= current_chunk_clone.len() {
                                    let next_chunk_index = current_chunk_index() + 1;
                                    if next_chunk_index >= nb_chunks_to_write {
                                        current_word_in_chunk_index.set(next_word_index);
                                        duration_seconds.set(
                                            started_at()
                                                .map(|start| {
                                                    (get_timestamp_seconds_now_wasm() - start)
                                                        .clamp(1, 600)
                                                })
                                                .unwrap_or(1),
                                        );
                                        running.set(false);
                                        finished.set(true);
                                    } else {
                                        current_word_in_chunk_index.set(0);
                                        current_chunk_index.set(next_chunk_index);
                                        user_words.set(vec![]);
                                    }
                                } else {
                                    current_word_in_chunk_index.set(next_word_index);
                                }
                                current_text.set(String::new());
                            } else {
                                current_text.set(data.clone());
                            }
                        }
                    },
                    value: "{current_text}",
                    autofocus: true,
                    autocomplete: "off",
                    autocapitalize: "off",
                    spellcheck: "false",
                    aria_label: "Type the highlighted word",
                }
            } else {
                section { id: "results", class: "{results_class}",
                    h2 { "Run complete" }
                    if new_personal_best() {
                        p { class: "record-banner", role: "status", "New personal best!" }
                    }
                    div { class: "result-grid",
                        ResultStat { label: "WPM", value: format!("{wpm:.0}") }
                        ResultStat {
                            label: "Accuracy",
                            value: format!("{accuracy_percent:.0}%"),
                        }
                        ResultStat { label: "Score", value: score.to_string() }
                        ResultStat {
                            label: "Time",
                            value: format!("{}s", duration_seconds()),
                        }
                    }
                    p { class: "combo-summary", "Best combo: 🔥 {max_combo}" }
                    if let Some(badge) = new_badge() {
                        section { class: "new-badge", aria_live: "polite",
                            span { "New badge" }
                            strong { "{badge.title()}" }
                            p { "{badge.description()}" }
                        }
                    }
                    if !stats.earned_badges.is_empty() {
                        div { class: "badge-shelf", aria_label: "Earned badges",
                            for badge in stats.earned_badges.iter().copied() {
                                AchievementBadge { badge }
                            }
                        }
                    }
                    Button {
                        size: ButtonSize::Lg,
                        onclick: move |_| {
                            current_chunk_index.set(0);
                            current_word_in_chunk_index.set(0);
                            current_text.set(String::new());
                            user_words.set(Vec::new());
                            started_at.set(None);
                            run_id.set(String::new());
                            correct_words.set(0);
                            wrong_words.set(0);
                            duration_seconds.set(0);
                            timer_value.set(TEST_DURATION_SECONDS);
                            running.set(false);
                            finished.set(false);
                            submitted_run.set(None);
                            save_message.set(String::new());
                            combo.set(0);
                            max_combo.set(0);
                            combo_milestone.set(None);
                            processed_gamification_run.set(None);
                            new_badge.set(None);
                            new_personal_best.set(false);
                        },
                        "Try again"
                    }
                    if !save_message().is_empty() {
                        p { class: "save-message", "{save_message}" }
                    } else if profile.is_none() {
                        p { class: "save-message",
                            "Sign in with GitHub to keep future results."
                        }
                    }
                }

                if let Some(private_profile) = profile.clone() {
                    HistoryPanel { profile: private_profile }
                }

                LeaderboardPanel {
                    board: leaderboard_resource().flatten(),
                    scope: leaderboard_scope(),
                    selected_day: leaderboard_day(),
                    recent_days: recent_days_resource()
                        .unwrap_or_else(|| vec![leaderboard_day()]),
                    on_scope: move |scope| leaderboard_scope.set(scope),
                    on_day: move |day| leaderboard_day.set(day),
                }

                div { class: "sources",
                    div { "Text sources" }
                    for source in story.sources {
                        a { href: source, target: "_blank", rel: "noreferrer", "{last_title}" }
                    }
                }
            }
        }
    }
}

#[component]
fn ProfileBar(profile: Option<PrivateProfile>) -> Element {
    rsx! {
        header { class: "profile-bar",
            if let Some(private_profile) = profile {
                div { class: "profile-identity",
                    ImageAvatar {
                        size: AvatarImageSize::Small,
                        src: private_profile.user.avatar_url.clone(),
                        alt: format!("{}'s GitHub avatar", private_profile.user.login),
                        "{private_profile.user.login.chars().next().unwrap_or('?')}"
                    }
                    span { "@{private_profile.user.login}" }
                }
                form { action: "/auth/logout", method: "post",
                    Button {
                        r#type: "submit",
                        size: ButtonSize::Sm,
                        variant: ButtonVariant::Ghost,
                        "Log out"
                    }
                }
            } else {
                form { action: "/auth/github", method: "get",
                    Button {
                        r#type: "submit",
                        size: ButtonSize::Sm,
                        variant: ButtonVariant::Outline,
                        "Log in with GitHub"
                    }
                }
            }
        }
    }
}

#[component]
fn ResultStat(label: &'static str, value: String) -> Element {
    rsx! {
        div { class: "result-stat",
            strong { "{value}" }
            span { "{label}" }
        }
    }
}

#[component]
fn AchievementBadge(badge: Badge) -> Element {
    rsx! {
        div { class: "achievement-badge", title: "{badge.description()}",
            span { aria_hidden: "true", "✦" }
            strong { "{badge.title()}" }
        }
    }
}

#[component]
fn HistoryPanel(profile: PrivateProfile) -> Element {
    rsx! {
        section { class: "history-panel",
            h2 { "Private profile" }
            div { class: "profile-summary",
                span { "{profile.user.total_runs} runs" }
                span { "Best {profile.user.best_wpm:.0} WPM" }
                span { "Best score {profile.user.best_score}" }
            }
            if profile.history.is_empty() {
                p { "Your completed runs will appear here." }
            } else {
                div { class: "history-list",
                    for result in profile.history.iter() {
                        div { class: "history-row", key: "{result.run_id}",
                            div {
                                strong { "{result.score} pts" }
                                span { "{result.story_title}" }
                            }
                            div { class: "history-metrics",
                                span { "{result.wpm:.0} WPM" }
                                span { "{result.accuracy * 100.0:.0}%" }
                                span { "{result.created_at.format(\"%Y-%m-%d\")}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn LeaderboardPanel(
    board: Option<Leaderboard>,
    scope: LeaderboardScope,
    selected_day: String,
    recent_days: Vec<String>,
    on_scope: EventHandler<LeaderboardScope>,
    on_day: EventHandler<String>,
) -> Element {
    rsx! {
        section { class: "leaderboard-panel",
            h2 { "Leaderboard" }
            div { class: "leaderboard-tabs", role: "tablist", aria_label: "Leaderboard period",
                button {
                    class: if scope == LeaderboardScope::Day { "leaderboard-tab active" } else { "leaderboard-tab" },
                    r#type: "button",
                    role: "tab",
                    aria_selected: scope == LeaderboardScope::Day,
                    onclick: move |_| on_scope.call(LeaderboardScope::Day),
                    "Day"
                }
                button {
                    class: if scope == LeaderboardScope::Week { "leaderboard-tab active" } else { "leaderboard-tab" },
                    r#type: "button",
                    role: "tab",
                    aria_selected: scope == LeaderboardScope::Week,
                    onclick: move |_| on_scope.call(LeaderboardScope::Week),
                    "Week"
                }
                button {
                    class: if scope == LeaderboardScope::Global { "leaderboard-tab active" } else { "leaderboard-tab" },
                    r#type: "button",
                    role: "tab",
                    aria_selected: scope == LeaderboardScope::Global,
                    onclick: move |_| on_scope.call(LeaderboardScope::Global),
                    "Global"
                }
            }
            if scope == LeaderboardScope::Day {
                div { class: "leaderboard-day-picker",
                    label { r#for: "leaderboard-day", "Challenge day" }
                    select {
                        id: "leaderboard-day",
                        value: "{selected_day}",
                        onchange: move |event| on_day.call(event.value()),
                        for day in recent_days.iter() {
                            option { value: "{day}", selected: *day == selected_day, "{day}" }
                        }
                    }
                }
            }
            if let Some(board) = board {
                p { class: "leaderboard-label", "{board.label}" }
                if board.entries.is_empty() {
                    p { class: "leaderboard-empty", "No ranked runs yet. Be the first." }
                } else {
                    div { class: "leaderboard-list",
                        for entry in board.entries.iter() {
                            div { class: "leaderboard-row", key: "{entry.github_id}-{entry.run_id}",
                                span { class: "leaderboard-rank", "#{entry.rank}" }
                                ImageAvatar {
                                    size: AvatarImageSize::Small,
                                    src: entry.avatar_url.clone(),
                                    alt: format!("{}'s GitHub avatar", entry.login),
                                    "{entry.login.chars().next().unwrap_or('?')}"
                                }
                                div { class: "leaderboard-identity",
                                    strong { "@{entry.login}" }
                                    span { "{entry.score} pts" }
                                }
                                div { class: "leaderboard-metrics",
                                    span { "{entry.wpm:.0} WPM" }
                                    span { "{entry.accuracy * 100.0:.0}%" }
                                }
                            }
                        }
                    }
                }
            } else {
                p { class: "leaderboard-empty", "Loading rankings…" }
            }
        }
    }
}
