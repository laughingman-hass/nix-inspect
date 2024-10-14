use std::{
	collections::HashMap,
	fmt,
	ops::{Deref, DerefMut},
};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{text::Text, widgets::ListState};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{workers::NixValue, Config};

#[derive(Default, Debug)]
pub struct Model {
	pub running_state: RunningState,

	pub path_data: PathDataMap,
	pub recents: Vec<BrowserPath>,

	pub config: Config,

	pub visit_stack: BrowserStack,

	pub search_input: InputState,
	pub path_navigator_input: InputState,
	pub new_bookmark_input: InputState,

	/// TODO: things that the architecture doesnt handle all that well
	pub prev_tab_completion: Option<String>,

	pub root_view_state: ListState,
	pub bookmark_view_state: ListState,
	pub recents_view_state: ListState,
}

impl Model {
	pub fn selected_bookmark(&self) -> Option<&Bookmark> {
		self.bookmark_view_state
			.selected()
			.and_then(|i| self.config.bookmarks.get(i))
	}

	pub fn selected_recent(&self) -> Option<&BrowserPath> {
		self.recents_view_state
			.selected()
			.and_then(|i| self.recents.get(i))
	}

	/// Update the selection of the parent to match the current path
	pub fn update_parent_selection(&mut self, current_path: BrowserPath) {
		let mut new_stack = vec![];
		let mut path = current_path;
		new_stack.push(BrowserStackItem::BrowserPath(path.clone()));
		while let Some(parent) = path.parent() {
			new_stack.push(BrowserStackItem::BrowserPath(parent.clone()));
			if let Some(PathData::List(list)) = self.path_data.get_mut(&parent) {
				if let Some(pos) = list.list.iter().position(|x| x == path.0.last().unwrap()) {
					list.state.select(Some(pos));
				}
			}
			path = parent;
		}
		new_stack.push(BrowserStackItem::Root);
		self.root_view_state.select(Some(2));
		new_stack.reverse();
		*self.visit_stack = new_stack;
	}
}

#[derive(Default, Debug)]
pub struct PathDataMap(HashMap<BrowserPath, PathData>);

impl Deref for PathDataMap {
	type Target = HashMap<BrowserPath, PathData>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
impl DerefMut for PathDataMap {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl PathDataMap {
	pub fn current_list(&self, current_path: &BrowserPath) -> Option<&ListData> {
		self.get(current_path).and_then(|x| match x {
			PathData::List(data) => Some(data),
			_ => None,
		})
	}
	pub fn current_list_mut(&mut self, current_path: &BrowserPath) -> Option<&mut ListData> {
		self.get_mut(&current_path).and_then(|x| match x {
			PathData::List(data) => Some(data),
			_ => None,
		})
	}
}

#[derive(Debug, Default)]
pub struct BrowserStack(pub Vec<BrowserStackItem>);

impl Deref for BrowserStack {
	type Target = Vec<BrowserStackItem>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
impl DerefMut for BrowserStack {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl BrowserStack {
	pub fn push_path(&mut self, path: BrowserPath) {
		self.0.push(BrowserStackItem::BrowserPath(path))
	}
	pub fn prev_item(&self) -> Option<&BrowserStackItem> {
		self.0.len().checked_sub(2).and_then(|i| self.0.get(i))
	}
	pub fn current(&self) -> Option<&BrowserPath> {
		match self.0.last() {
			Some(BrowserStackItem::BrowserPath(p)) => Some(p),
			_ => None,
		}
	}
	pub fn current_force(&self) -> &BrowserPath {
		match self.0.last() {
			Some(BrowserStackItem::BrowserPath(p)) => p,
			_ => panic!("current visit stack item is not a path"),
		}
	}
}

#[derive(Debug)]
pub enum Message {
	TermEvent(crossterm::event::Event),
	Data(BrowserPath, PathData),
	CurrentPath(BrowserPath),
	Refresh,
	PageDown,
	PageUp,
	SearchEnter,
	SearchExit,
	SearchInput(KeyEvent),
	NavigatorEnter,
	NavigatorExit,
	NavigatorInput(KeyEvent),
	BookmarkInputEnter,
	BookmarkInputExit,
	BookmarkInput(KeyEvent),
	CreateBookmark,
	DeleteBookmark,
	Back,
	EnterItem,
	ListUp,
	ListDown,
	SearchNext,
	SearchPrev,
	NavigatorNext,
	NavigatorPrev,
	Quit,
}

#[derive(Debug, Clone)]
pub enum BrowserStackItem {
	Root,
	Bookmarks,
	Recents,
	BrowserPath(BrowserPath),
}

#[derive(Debug, Default, Eq, Hash, PartialEq, Clone)]
pub struct BrowserPath(pub Vec<String>);

impl Serialize for BrowserPath {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		serializer.collect_str(&self.to_expr())
	}
}

impl<'de> Deserialize<'de> for BrowserPath {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		let s = String::deserialize(deserializer)?;
		Ok(BrowserPath::from(s))
	}
}

impl BrowserPath {
	pub fn parent(&self) -> Option<BrowserPath> {
		if self.0.len() > 1 {
			Some(BrowserPath(self.0[..self.0.len() - 1].to_vec()))
		} else {
			None
		}
	}
	pub fn child(&self, name: String) -> BrowserPath {
		let mut clone = self.0.clone();
		clone.push(name);
		BrowserPath(clone)
	}
	pub fn extend(mut self, other: &BrowserPath) -> BrowserPath {
		self.0.extend_from_slice(&other.0);
		self
	}
	pub fn to_expr(&self) -> String {
		let mut result = String::new();

		let mut items = self.0.iter().peekable();

		if let Some(0) = items.peek().map(|x| x.len()) {
			items.next();
		}

		for (i, element) in items.enumerate() {
			if i > 0 {
				result.push('.');
			}

			if element.contains('.') {
				result.push('"');
				result.push_str(element);
				result.push('"');
			} else {
				result.push_str(element);
			}
		}

		result
	}
}

impl From<String> for BrowserPath {
	fn from(value: String) -> Self {
		let mut res = Vec::new();
		let mut cur = String::new();
		let mut chars = value.chars().peekable();

		while let Some(c) = chars.next() {
			match c {
				'.' => {
					res.push(cur);
					cur = String::new();
				}
				'"' => {
					while let Some(inner_c) = chars.next() {
						if inner_c == '"' {
							break;
						}
						cur.push(inner_c);
					}
				}
				_ => cur.push(c),
			}
		}

		res.push(cur);

		BrowserPath(res)
	}
}

#[test]
pub fn test_expr_conversion() {
	let path = BrowserPath::from(".".to_string());
	assert_eq!(path.to_expr(), "");
	let path = BrowserPath::from(".darwinConfigurations".to_string());
	assert_eq!(path.to_expr(), "darwinConfigurations");
	let path = BrowserPath::from(r#".darwinConfigurations."example.com""#.to_string());
	assert_eq!(path.to_expr(), r#"darwinConfigurations."example.com""#);
}

#[derive(Default, Debug, PartialEq, Eq)]
pub enum RunningState {
	#[default]
	Running,
	Stopped,
}

#[derive(Debug, Clone)]
pub enum ListType {
	List,
	Attrset,
}

#[derive(Debug, Clone)]
pub struct ListData {
	pub state: ListState,
	pub list_type: ListType,
	pub list: Vec<String>,
}

impl ListData {
	pub fn selected(&self, current_path: &BrowserPath) -> Option<BrowserPath> {
		self.state
			.selected()
			.and_then(|i| self.list.get(i))
			.map(|x| current_path.child(x.to_string()))
	}
}

#[derive(Debug, Clone)]
pub enum PathData {
	List(ListData),
	Thunk,
	Int(i64),
	Float(f64),
	Bool(bool),
	String(String),
	Path(String),
	Null,
	Function,
	External,
	Loading,
	Error(String),
}

impl fmt::Display for PathData {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			PathData::List(list_data) => write!(f, "{:?}", list_data),
			PathData::Thunk => write!(f, "Thunk"),
			PathData::Int(value) => write!(f, "{}", value),
			PathData::Float(value) => write!(f, "{}", value),
			PathData::Bool(value) => write!(f, "{}", value),
			PathData::String(value) => write!(f, "\"{}\"", value),
			PathData::Path(value) => write!(f, "Path(\"{}\")", value),
			PathData::Null => write!(f, "Null"),
			PathData::Function => write!(f, "Function"),
			PathData::External => write!(f, "External"),
			PathData::Loading => write!(f, "Loading"),
			PathData::Error(reason) => write!(f, "{}", reason),
		}
	}
}

impl From<NixValue> for PathData {
	fn from(value: NixValue) -> Self {
		match value {
			NixValue::Thunk => PathData::Thunk,
			NixValue::Int(i) => PathData::Int(i),
			NixValue::Float(f) => PathData::Float(f),
			NixValue::Bool(b) => PathData::Bool(b),
			NixValue::String(s) => PathData::String(s),
			NixValue::Path(p) => PathData::Path(p),
			NixValue::Null => PathData::Null,
			NixValue::Attrs(attrs) => PathData::List(ListData {
				list_type: ListType::Attrset,
				state: ListState::default().with_selected(Some(0)),
				list: attrs,
			}),
			NixValue::List(size) => PathData::List(ListData {
				list_type: ListType::List,
				state: ListState::default().with_selected(Some(0)),
				list: (0..size).map(|i| format!("{}", i)).collect(),
			}),
			NixValue::Function => PathData::Function,
			NixValue::External => PathData::External,
			NixValue::Error(e) => PathData::Error(e),
		}
	}
}

impl PathData {
	pub fn get_type(&self) -> String {
		match self {
			PathData::List(data) => match data.list_type {
				ListType::Attrset => "Attrset",
				ListType::List => "List",
			},
			PathData::Thunk => "Thunk",
			PathData::Int(_) => "Int",
			PathData::Float(_) => "Float",
			PathData::Bool(_) => "Bool",
			PathData::String(_) => "String",
			PathData::Path(_) => "Path",
			PathData::Null => "Null",
			PathData::Function => "Function",
			PathData::External => "External",
			PathData::Loading => "Loading",
			PathData::Error(_) => "Error",
		}
		.to_string()
	}
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Bookmark {
	pub display: String,
	pub path: BrowserPath,
}

impl<'a> Into<Text<'a>> for Bookmark {
	fn into(self) -> Text<'a> {
		Text::raw(self.display)
	}
}

#[derive(Debug, Default)]
pub enum InputState {
	#[default]
	Normal,
	Active(InputModel),
}

#[derive(Debug)]
pub struct InputModel {
	pub typing: bool,
	pub input: String,
	pub cursor_position: usize,
}

impl InputModel {
	pub fn handle_key_event(&mut self, key: KeyEvent) {
		match key.code {
			KeyCode::Char(c) => {
				self.insert(c);
			}
			KeyCode::Backspace => {
				self.backspace();
			}
			KeyCode::Left => {
				self.move_cursor_left();
			}
			KeyCode::Right => {
				self.move_cursor_right();
			}
			_ => {}
		}
	}

	pub fn insert(&mut self, c: char) {
		self.input.insert(self.cursor_position, c);
		self.cursor_position += 1;
	}

	pub fn backspace(&mut self) {
		if self.cursor_position == 0 {
			return;
		}

		let current_index = self.cursor_position;
		let from_left_to_current_index = current_index - 1;
		let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
		let after_char_to_delete = self.input.chars().skip(current_index);
		self.input = before_char_to_delete.chain(after_char_to_delete).collect();
		self.move_cursor_left();
	}

	pub fn move_cursor_left(&mut self) {
		self.cursor_position = self.clamp_cursor(self.cursor_position - 1);
	}

	pub fn move_cursor_right(&mut self) {
		self.cursor_position = self.clamp_cursor(self.cursor_position + 1);
	}

	fn clamp_cursor(&mut self, pos: usize) -> usize {
		pos.clamp(0, self.input.len())
	}
}

pub fn next(i: usize, len: usize) -> usize {
	if i >= len - 1 {
		0
	} else {
		i + 1
	}
}

pub fn select_next(list_state: &mut ListState, len: usize) {
	list_state.select(list_state.selected().map(|i| next(i, len)).or(Some(0)));
}

pub fn prev(i: usize, len: usize) -> usize {
	if i == 0 {
		len - 1
	} else {
		i - 1
	}
}

pub fn select_prev(list_state: &mut ListState, len: usize) {
	list_state.select(list_state.selected().map(|i| prev(i, len)).or(Some(0)));
}
