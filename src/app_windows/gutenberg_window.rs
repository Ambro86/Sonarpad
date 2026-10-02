use reqwest::blocking::Client;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    IDYES, MB_ICONQUESTION, MB_YESNO, MESSAGEBOX_STYLE, MessageBoxW,
};
use windows::core::PCWSTR;

use crate::accessibility::to_wide;
use crate::app_windows::prompt_window::{self, PromptDirectoryOptions};
use crate::app_windows::youtube_transcript_window::{
    self, MultilineSearchOptions, MultilineSelectionItem, MultilineSelectionResult,
};
use crate::settings::Language;
use crate::{editor_manager, i18n, show_error, with_state};

const SEARCH_URL: &str = "https://sonarpad.com/api/gutenberg/search.php";
const DOWNLOAD_URL: &str = "https://sonarpad.com/api/gutenberg/download.php";
const PAGE_SIZE: usize = 20;
const PREVIOUS_RESULTS_ID: &str = "__sonarpad_gutenberg_previous_results__";
const NEXT_RESULTS_ID: &str = "__sonarpad_gutenberg_next_results__";
const USER_AGENT: &str = concat!("Sonarpad/", env!("CARGO_PKG_VERSION"));

#[derive(Clone)]
struct GutenbergClient {
    client: Client,
}

#[derive(Debug, Clone, Deserialize)]
struct GutenbergBook {
    #[serde(default)]
    id: i64,
    #[serde(default = "default_book_title")]
    title: String,
    #[serde(default)]
    authors: Vec<GutenbergAuthor>,
    #[serde(default)]
    languages: Vec<String>,
    #[serde(default)]
    summaries: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct GutenbergAuthor {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct GutenbergPage {
    #[serde(default)]
    next: Option<String>,
    #[serde(default)]
    previous: Option<String>,
    #[serde(default)]
    results: Vec<GutenbergBook>,
}

#[derive(Clone)]
struct GutenbergSearch {
    query: String,
    language_code: String,
}

#[derive(Clone)]
struct GutenbergReturnContext {
    search: GutenbergSearch,
    books: Vec<GutenbergBook>,
    previous_page: Option<String>,
    next_page: Option<String>,
    selected_id: String,
    opened_path: Option<PathBuf>,
}

static GUTENBERG_RETURN_CONTEXT: OnceLock<Mutex<Option<GutenbergReturnContext>>> = OnceLock::new();

fn return_context() -> &'static Mutex<Option<GutenbergReturnContext>> {
    GUTENBERG_RETURN_CONTEXT.get_or_init(|| Mutex::new(None))
}

fn remember_return_context(context: GutenbergReturnContext) {
    if let Ok(mut stored) = return_context().lock() {
        *stored = Some(context);
    }
}

fn clear_return_context() {
    if let Ok(mut stored) = return_context().lock() {
        *stored = None;
    }
}

pub(crate) fn current_document_has_return_context(parent: HWND) -> bool {
    let opened_path = return_context()
        .lock()
        .ok()
        .and_then(|stored| stored.as_ref()?.opened_path.clone());
    let Some(opened_path) = opened_path else {
        return false;
    };
    with_state(parent, |state| {
        state
            .docs
            .get(state.current)
            .and_then(|document| document.path.as_ref())
            .is_some_and(|path| path == &opened_path)
    })
    .unwrap_or(false)
}

pub(crate) fn reopen_results(parent: HWND) {
    let Some(context) = return_context()
        .lock()
        .ok()
        .and_then(|stored| stored.clone())
    else {
        return;
    };
    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
    browse_return_results(parent, language, context);
}

fn default_book_title() -> String {
    "Project Gutenberg".to_string()
}

impl Default for GutenbergClient {
    fn default() -> Self {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(120))
            .build()
            .unwrap_or_else(|error| {
                crate::log_debug(&format!("Gutenberg HTTP client setup failed: {error}"));
                Client::new()
            });
        Self { client }
    }
}

impl GutenbergClient {
    fn search(
        &self,
        search: &GutenbergSearch,
        page_url: Option<&str>,
    ) -> Result<GutenbergPage, String> {
        let page_size = PAGE_SIZE.to_string();
        let request = if let Some(url) = page_url {
            self.client.get(resolve_page_url(url))
        } else {
            self.client.get(SEARCH_URL).query(&[
                ("q", search.query.trim()),
                ("lang", search.language_code.as_str()),
                ("page_size", page_size.as_str()),
            ])
        };

        let response = request
            .header("Accept", "application/json")
            .send()
            .map_err(|error| format!("Gutenberg network error: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Gutenberg server error: {error}"))?;

        let mut page = response
            .json::<GutenbergPage>()
            .map_err(|error| format!("Invalid Gutenberg response: {error}"))?;
        page.results
            .retain(|book| book.id > 0 && !book.title.trim().is_empty());
        Ok(page)
    }

    fn download_epub(&self, book_id: i64) -> Result<Vec<u8>, String> {
        let response = self
            .client
            .get(DOWNLOAD_URL)
            .query(&[("id", book_id.to_string()), ("format", "epub".to_string())])
            .send()
            .map_err(|error| format!("Gutenberg download error: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Gutenberg download error: {error}"))?;
        response
            .bytes()
            .map(|bytes| bytes.to_vec())
            .map_err(|error| format!("Gutenberg download error: {error}"))
    }
}

pub fn open(parent: HWND) {
    let language = with_state(parent, |state| state.settings.language).unwrap_or_default();
    let Some(mut search) = prompt_search(parent, language, None) else {
        return;
    };
    let client = GutenbergClient::default();

    loop {
        crate::screen_reader_speak(&i18n::tr(language, "gutenberg.loading"));
        let mut page = match client.search(&search, None) {
            Ok(page) => page,
            Err(error) => {
                show_catalog_error(parent, language, "gutenberg.error", &error);
                return;
            }
        };
        let mut books = page.results;
        let mut previous_page = page.previous.take();
        let mut next_page = page.next.take();
        let mut selected_id = None;

        if books.is_empty() {
            show_error(
                parent,
                language,
                &i18n::tr(language, "gutenberg.no_results"),
            );
            let Some(new_search) = prompt_search(parent, language, Some(&search)) else {
                return;
            };
            search = new_search;
            continue;
        }

        loop {
            let items = gutenberg_result_items(
                &books,
                language,
                previous_page.is_some(),
                next_page.is_some(),
            );
            let result = youtube_transcript_window::select_multiline_items_with_search_without_parent_restore_on_action(
                parent,
                language,
                i18n::tr(language, "gutenberg.title"),
                items,
                selected_id.clone(),
                MultilineSearchOptions {
                    initial_query: search.query.clone(),
                    search_button_label: i18n::tr(language, "podcasts.search.button"),
                    show_search_edit: true,
                    secondary_action_label: None,
                    context_actions: Vec::new(),
                    right_arrow_accepts_selection: true,
                    left_arrow_closes: true,
                    escape_stops_active_player: false,
                    refresh: None,
                },
            );

            match result {
                MultilineSelectionResult::Cancelled => return,
                MultilineSelectionResult::Search(query) => {
                    search.query = query.trim().to_string();
                    break;
                }
                MultilineSelectionResult::SecondaryAction => {}
                MultilineSelectionResult::Selected(id) => {
                    if id == PREVIOUS_RESULTS_ID || id == NEXT_RESULTS_ID {
                        let page_url = if id == PREVIOUS_RESULTS_ID {
                            previous_page.clone()
                        } else {
                            next_page.clone()
                        };
                        let Some(page_url) = page_url else {
                            continue;
                        };
                        crate::screen_reader_speak(&i18n::tr(language, "gutenberg.loading"));
                        match client.search(&search, Some(&page_url)) {
                            Ok(loaded) if !loaded.results.is_empty() => {
                                books = loaded.results;
                                previous_page = loaded.previous;
                                next_page = loaded.next;
                                selected_id = Some(first_gutenberg_result_id(
                                    &books,
                                    previous_page.is_some(),
                                    next_page.is_some(),
                                ));
                            }
                            Ok(_) => {
                                if id == NEXT_RESULTS_ID {
                                    next_page = None;
                                }
                            }
                            Err(error) => {
                                show_catalog_error(parent, language, "gutenberg.error", &error)
                            }
                        }
                        continue;
                    }
                    selected_id = Some(id.clone());
                    if let Some(book) = books.iter().find(|book| book.id.to_string() == id) {
                        let context = GutenbergReturnContext {
                            search: search.clone(),
                            books: books.clone(),
                            previous_page: previous_page.clone(),
                            next_page: next_page.clone(),
                            selected_id: id,
                            opened_path: None,
                        };
                        if show_book(parent, language, &client, book, context) {
                            return;
                        }
                    }
                }
            }
        }
    }
}

fn browse_return_results(parent: HWND, language: Language, mut context: GutenbergReturnContext) {
    let client = GutenbergClient::default();
    loop {
        let items = gutenberg_result_items(
            &context.books,
            language,
            context.previous_page.is_some(),
            context.next_page.is_some(),
        );
        match youtube_transcript_window::select_multiline_items_with_search_without_parent_restore_on_action(
            parent,
            language,
            i18n::tr(language, "gutenberg.title"),
            items,
            Some(context.selected_id.clone()),
            MultilineSearchOptions {
                initial_query: context.search.query.clone(),
                search_button_label: i18n::tr(language, "podcasts.search.button"),
                show_search_edit: true,
                secondary_action_label: None,
                context_actions: Vec::new(),
                right_arrow_accepts_selection: true,
                left_arrow_closes: true,
                escape_stops_active_player: false,
                refresh: None,
            },
        ) {
            MultilineSelectionResult::Cancelled => {
                clear_return_context();
                return;
            }
            MultilineSelectionResult::Search(query) => {
                let mut search = context.search.clone();
                search.query = query.trim().to_string();
                crate::screen_reader_speak(&i18n::tr(language, "gutenberg.loading"));
                match client.search(&search, None) {
                    Ok(page) if !page.results.is_empty() => {
                        context.search = search;
                        context.books = page.results;
                        context.previous_page = page.previous;
                        context.next_page = page.next;
                        context.selected_id = context.books[0].id.to_string();
                    }
                    Ok(_) => show_error(
                        parent,
                        language,
                        &i18n::tr(language, "gutenberg.no_results"),
                    ),
                    Err(error) => show_catalog_error(parent, language, "gutenberg.error", &error),
                }
            }
            MultilineSelectionResult::SecondaryAction => {}
            MultilineSelectionResult::Selected(id) => {
                if id == PREVIOUS_RESULTS_ID || id == NEXT_RESULTS_ID {
                    let page_url = if id == PREVIOUS_RESULTS_ID {
                        context.previous_page.clone()
                    } else {
                        context.next_page.clone()
                    };
                    let Some(page_url) = page_url else {
                        continue;
                    };
                    crate::screen_reader_speak(&i18n::tr(language, "gutenberg.loading"));
                    match client.search(&context.search, Some(&page_url)) {
                        Ok(page) if !page.results.is_empty() => {
                            context.books = page.results;
                            context.previous_page = page.previous;
                            context.next_page = page.next;
                            context.selected_id = first_gutenberg_result_id(
                                &context.books,
                                context.previous_page.is_some(),
                                context.next_page.is_some(),
                            );
                        }
                        Ok(_) => {
                            if id == NEXT_RESULTS_ID {
                                context.next_page = None;
                            }
                        }
                        Err(error) => {
                            show_catalog_error(parent, language, "gutenberg.error", &error)
                        }
                    }
                    continue;
                }
                context.selected_id = id.clone();
                if let Some(book) = context
                    .books
                    .iter()
                    .find(|book| book.id.to_string() == id)
                    .cloned()
                    && show_book(parent, language, &client, &book, context.clone())
                {
                    return;
                }
            }
        }
    }
}

fn gutenberg_result_items(
    books: &[GutenbergBook],
    language: Language,
    has_previous: bool,
    has_more: bool,
) -> Vec<MultilineSelectionItem> {
    let mut items = Vec::with_capacity(books.len().saturating_add(2));
    if has_previous {
        items.push(MultilineSelectionItem {
            id: PREVIOUS_RESULTS_ID.to_string(),
            title: i18n::tr(language, "podcasts.categories.previous_results"),
            description: None,
        });
    }
    items.extend(books.iter().map(|book| MultilineSelectionItem {
        id: book.id.to_string(),
        title: book.title.clone(),
        description: Some(book_description(book, language)),
    }));
    if has_more {
        items.push(MultilineSelectionItem {
            id: NEXT_RESULTS_ID.to_string(),
            title: i18n::tr(language, "podcasts.categories.next_results"),
            description: None,
        });
    }
    items
}

fn first_gutenberg_result_id(
    books: &[GutenbergBook],
    has_previous: bool,
    has_more: bool,
) -> String {
    if has_previous {
        PREVIOUS_RESULTS_ID.to_string()
    } else if let Some(book) = books.first() {
        book.id.to_string()
    } else if has_more {
        NEXT_RESULTS_ID.to_string()
    } else {
        String::new()
    }
}

fn prompt_search(
    parent: HWND,
    language: Language,
    previous: Option<&GutenbergSearch>,
) -> Option<GutenbergSearch> {
    let languages = gutenberg_languages();
    let default_code = previous
        .map(|value| value.language_code.as_str())
        .unwrap_or_else(|| default_language_code(language));
    let default_selection = languages
        .iter()
        .position(|(code, _)| *code == default_code)
        .unwrap_or(1);
    let result = prompt_window::prompt_directory_search(
        parent,
        PromptDirectoryOptions {
            title: i18n::tr(language, "gutenberg.title"),
            type_label: i18n::tr(language, "gutenberg.language"),
            options: languages
                .iter()
                .map(|(code, label)| format!("{label} ({code})"))
                .collect(),
            default_selection,
            secondary_type_label: String::new(),
            secondary_options: Vec::new(),
            secondary_default_selection: 0,
            tertiary_type_label: String::new(),
            tertiary_options: Vec::new(),
            tertiary_default_selection: 0,
            tertiary_options_primary_index_only: None,
            quaternary_type_label: String::new(),
            quaternary_options: Vec::new(),
            quaternary_default_selection: 0,
            focus_primary_field: true,
            primary_label: i18n::tr(language, "gutenberg.search_prompt"),
            primary_labels: Vec::new(),
            primary_default: previous
                .map(|value| value.query.clone())
                .unwrap_or_default(),
            secondary_label: String::new(),
            secondary_default: String::new(),
            tertiary_label: String::new(),
            tertiary_default: String::new(),
            checkbox_label: String::new(),
            checkbox_default: false,
        },
        language,
    )?;
    let (code, _label) = languages
        .get(result.selected_index)
        .copied()
        .unwrap_or(("en", "English"));
    Some(GutenbergSearch {
        query: result.primary_value.trim().to_string(),
        language_code: code.to_string(),
    })
}

fn show_book(
    parent: HWND,
    language: Language,
    client: &GutenbergClient,
    book: &GutenbergBook,
    context: GutenbergReturnContext,
) -> bool {
    let description = book_description(book, language);
    let result = youtube_transcript_window::select_multiline_items_with_search(
        parent,
        language,
        book.title.clone(),
        vec![MultilineSelectionItem {
            id: book.id.to_string(),
            title: book.title.clone(),
            description: Some(description),
        }],
        Some(book.id.to_string()),
        MultilineSearchOptions {
            initial_query: String::new(),
            search_button_label: String::new(),
            show_search_edit: false,
            secondary_action_label: Some(i18n::tr(language, "gutenberg.download")),
            context_actions: Vec::new(),
            right_arrow_accepts_selection: true,
            left_arrow_closes: true,
            escape_stops_active_player: false,
            refresh: None,
        },
    );

    if matches!(
        result,
        MultilineSelectionResult::Selected(_) | MultilineSelectionResult::SecondaryAction
    ) {
        return download_book(parent, language, client, book, context);
    }
    false
}

fn download_book(
    parent: HWND,
    language: Language,
    client: &GutenbergClient,
    book: &GutenbergBook,
    mut context: GutenbergReturnContext,
) -> bool {
    let _power_awake = crate::power_awake::acquire("download");
    crate::screen_reader_speak(&i18n::tr(language, "gutenberg.downloading"));
    let bytes = match client.download_epub(book.id) {
        Ok(bytes) => bytes,
        Err(error) => {
            show_catalog_error(parent, language, "gutenberg.download_error", &error);
            return false;
        }
    };
    let path = match save_epub(book, &bytes) {
        Ok(path) => path,
        Err(error) => {
            show_catalog_error(parent, language, "gutenberg.download_error", &error);
            return false;
        }
    };
    let message = i18n::tr_f(
        language,
        "gutenberg.saved_open_prompt",
        &[("path", path.to_string_lossy().as_ref())],
    );
    let response = unsafe {
        MessageBoxW(
            parent,
            PCWSTR(to_wide(&message).as_ptr()),
            PCWSTR(to_wide(&i18n::tr(language, "gutenberg.title")).as_ptr()),
            MESSAGEBOX_STYLE(MB_YESNO.0 | MB_ICONQUESTION.0),
        )
    };
    if response == IDYES {
        context.opened_path = Some(path.clone());
        remember_return_context(context);
        editor_manager::open_document(parent, &path);
        crate::restore_editor_focus(parent);
        return true;
    }
    false
}

fn save_epub(book: &GutenbergBook, bytes: &[u8]) -> Result<PathBuf, String> {
    let folder = PathBuf::from(crate::settings::default_documents_save_folder());
    fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let base_name = sanitize_file_name(&book.title);
    let preferred = folder.join(format!("{base_name} - Gutenberg {}.epub", book.id));
    let path = unique_path(&preferred);
    fs::write(&path, bytes).map_err(|error| error.to_string())?;
    Ok(path)
}

fn unique_path(preferred: &Path) -> PathBuf {
    if !preferred.exists() {
        return preferred.to_path_buf();
    }
    let parent = preferred.parent().unwrap_or_else(|| Path::new("."));
    let stem = preferred
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Gutenberg");
    let extension = preferred
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("epub");
    for index in 2..10_000 {
        let candidate = parent.join(format!("{stem} ({index}).{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!(
        "{stem}-{}.{}",
        chrono::Local::now().timestamp(),
        extension
    ))
}

fn sanitize_file_name(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(
            character,
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
        ) || character.is_control()
        {
            out.push('_');
        } else {
            out.push(character);
        }
    }
    let trimmed = out
        .trim()
        .trim_matches(|character| character == '.' || character == ' ');
    let mut result: String = trimmed.chars().take(120).collect();
    while result.ends_with('.') || result.ends_with(' ') {
        result.pop();
    }
    if result.is_empty() {
        "Project Gutenberg".to_string()
    } else {
        result
    }
}

fn book_description(book: &GutenbergBook, language: Language) -> String {
    let author = author_label(book, language);
    let languages = if book.languages.is_empty() {
        String::new()
    } else {
        format!(
            "{} {}",
            i18n::tr(language, "gutenberg.language_value"),
            book.languages.join(", ")
        )
    };
    let summary = book
        .summaries
        .first()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    [author.as_str(), languages.as_str(), summary]
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn author_label(book: &GutenbergBook, language: Language) -> String {
    let authors = book
        .authors
        .iter()
        .map(|author| author.name.trim())
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>();
    if authors.is_empty() {
        i18n::tr(language, "gutenberg.unknown_author")
    } else {
        authors.join(", ")
    }
}

fn show_catalog_error(parent: HWND, language: Language, key: &str, error: &str) {
    crate::log_debug(&format!("Gutenberg error: {error}"));
    let message = i18n::tr_f(language, key, &[("err", error)]);
    show_error(parent, language, &message);
}

fn resolve_page_url(value: &str) -> String {
    if value.starts_with("http://") || value.starts_with("https://") {
        return value.to_string();
    }
    url::Url::parse("https://sonarpad.com/")
        .and_then(|base| base.join(value))
        .map(|url| url.to_string())
        .unwrap_or_else(|_| format!("https://sonarpad.com/{value}"))
}

fn default_language_code(language: Language) -> &'static str {
    match language {
        Language::Italian => "it",
        Language::German => "de",
        Language::English => "en",
        Language::Spanish => "es",
        Language::French => "fr",
        Language::Portuguese | Language::PortugueseBrazilian => "pt",
        Language::Polish => "pl",
        _ => "en",
    }
}

fn gutenberg_languages() -> Vec<(&'static str, &'static str)> {
    vec![
        ("it", "Italiano"),
        ("en", "English"),
        ("es", "Español"),
        ("fr", "Français"),
        ("de", "Deutsch"),
        ("pt", "Português"),
        ("pl", "Polski"),
    ]
}
