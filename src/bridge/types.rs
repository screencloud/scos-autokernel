use super::expr::Terminal;
use super::Expr;
use std::borrow::Cow;
use std::ffi::CStr;
use std::fmt;
use std::fmt::Debug;
use std::str::FromStr;

use colored::Color;
use libc::{c_char, c_int, c_void};
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum ExprConvertError {
    #[error("encountered an expression of type None")]
    None,
    #[error("encountered a null expression")]
    NullExpr,
    #[error("encountered a null symbol")]
    NullSymbol,
    /*
    #[error("List expressions are not supported and shouldn't occur. Please report this as a bug if you encounter this message under normal use.")]
    List,
    */

    #[error("Range expressions are not supported and shouldn't occur. Please report this as a bug if you encounter this message under normal use.")]
    Range,
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Copy)]
#[repr(C)]
#[allow(dead_code)]
pub enum Tristate {
    No,
    Mod,
    Yes,
}

impl Tristate {
    pub fn invert(self) -> Self {
        match self {
            Tristate::No => Tristate::Yes,
            Tristate::Mod => Tristate::Mod,
            Tristate::Yes => Tristate::No,
        }
    }

    pub fn color(self) -> Color {
        match self {
            Tristate::No => Color::Red,
            Tristate::Mod => Color::Yellow,
            Tristate::Yes => Color::Green,
        }
    }
}

impl From<bool> for Tristate {
    fn from(value: bool) -> Self {
        if value {
            Tristate::Yes
        } else {
            Tristate::No
        }
    }
}

impl FromStr for Tristate {
    type Err = ();
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "n" => Ok(Tristate::No),
            "m" => Ok(Tristate::Mod),
            "y" => Ok(Tristate::Yes),
            _ => Err(()),
        }
    }
}

impl fmt::Display for Tristate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tristate::No => write!(f, "n"),
            Tristate::Mod => write!(f, "m"),
            Tristate::Yes => write!(f, "y"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, strum::AsRefStr)]
#[repr(C)]
#[allow(dead_code)]
pub enum SymbolType {
    Unknown,
    Boolean,
    Tristate,
    Int,
    Hex,
    String,
}

#[derive(Debug)]
#[repr(C)]
#[allow(dead_code)]
enum PropertyType {
    Unknown,
    Prompt,  /* prompt "foo prompt" or "BAZ Value" */
    Comment, /* text associated with a comment */
    Menu,    /* prompt associated with a menu or menuconfig symbol */
    Default, /* default y */
   /* Choice,*/  /* choice value */
    Select,  /* select BAR */
    Imply,   /* imply BAR */
    Range   /* range 7..100 (for a symbol) */
    /*Symbol,*/  /* where a symbol is defined */
}

#[repr(C)]
#[allow(dead_code)]
struct CProperty {
    next: *mut CProperty,
    prop_type: PropertyType,
    text: *const c_char,
    visible: CExprValue,
    expr: *mut c_void,
    menu: *mut c_void,
    file: *mut c_void,
    lineno: c_int,
}

#[repr(C)]
pub struct CSymbolValue {
    pub value: *mut c_void,
    pub tri: Tristate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolValue {
    Boolean(bool),
    Tristate(Tristate),
    Int(u64),
    Hex(u64),
    Number(u64),
    String(String),
    Auto(String),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(C)]
pub enum CExprType {
    None,
    Or,
    And,
    Not,
    Equal,
    Unequal,
    Lth,
    Leq,
    Gth,
    Geq,
    /*List,*/
    Symbol,
    Range,
}

#[repr(C)]
pub union CExprData {
    expression: *mut CExpr,
    symbol: *mut CSymbol,
    pub initdata: *mut c_void,
}

#[repr(C)]
pub struct CExpr {
    node: HlistNode, // Not needed
    expr_type: CExprType,
    val: Tristate,
    pub val_is_valid: bool,
    left: CExprData,
    right: CExprData,
}

impl CExpr {
    pub fn expr(&mut self) -> Result<Option<Expr>, ExprConvertError> {
        convert_expression(self)
    }
}

#[repr(C)]
pub struct CExprValue {
    expression: *mut CExpr,
    pub tri: Tristate,
}

impl CExprValue {
    pub fn expr(&self) -> Result<Option<Expr>, ExprConvertError> {
        convert_expression(self.expression)
    }
}

fn convert_expression(expression: *mut CExpr) -> Result<Option<Expr>, ExprConvertError> {

    macro_rules! expr {
        ($which: ident) => {
            if expression.is_null() {
                return Err(ExprConvertError::NullExpr);
            } else {
                Box::new(convert_expression(unsafe { (*expression).$which.expression })?.unwrap())
            }
        };
    }

    macro_rules! sym {
        ($which: ident) => {
            if expression.is_null() {
                return Err(ExprConvertError::NullSymbol);
            } else {
                unsafe {
                    (*expression).$which.symbol
                }
            }
        };
    }

    if expression.is_null() {
        return Ok(None);
    }


    Ok(Some(match unsafe { (*expression).expr_type } {
        CExprType::None => return Err(ExprConvertError::None),
        CExprType::Or => Expr::Or(expr!(left), expr!(right)),
        CExprType::And => Expr::And(expr!(left), expr!(right)),
        CExprType::Not => Expr::Not(expr!(left)),
        CExprType::Equal => Expr::Terminal(Terminal::Eq(sym!(left), sym!(right))),
        CExprType::Unequal => Expr::Terminal(Terminal::Neq(sym!(left), sym!(right))),
        CExprType::Lth => Expr::Terminal(Terminal::Lth(sym!(left), sym!(right))),
        CExprType::Leq => Expr::Terminal(Terminal::Leq(sym!(left), sym!(right))),
        CExprType::Gth => Expr::Terminal(Terminal::Gth(sym!(left), sym!(right))),
        CExprType::Geq => Expr::Terminal(Terminal::Geq(sym!(left), sym!(right))),
       /* CExprType::List => return Err(ExprConvertError::List), */
        CExprType::Symbol => { Expr::Terminal(Terminal::Symbol(sym!(left))) },
        CExprType::Range => return Err(ExprConvertError::Range),
    }))
}

#[repr(C)]
pub struct HlistNode {
    pub next: *mut HlistNode,
    pub pprev: *mut *mut HlistNode,
}

#[repr(C)]
pub struct CSymbol {
    pub node: HlistNode,
    pub name: *const c_char,
    pub symbol_type: SymbolType,
    pub current_value: CSymbolValue,
    default_values: [CSymbolValue; 5],
    pub visible: Tristate,

    menus: *const c_void,
    choice_link: *const c_void,


    pub flags: SymbolFlags,
    property: *mut CProperty,
    pub direct_dependencies:  CExprValue,
    pub reverse_dependencies:  CExprValue,
    pub implied:  CExprValue,
}

/*
struct symbol {
	/* link node for the hash table */
	struct hlist_node node;

	/* The name of the symbol, e.g. "FOO" for 'config FOO' */
	char *name;

	/* S_BOOLEAN, S_TRISTATE, ... */
	enum symbol_type type;

	/*
	 * The calculated value of the symbol. The SYMBOL_VALID bit is set in
	 * 'flags' when this is up to date. Note that this value might differ
	 * from the user value set in e.g. a .config file, due to visibility.
	 */
	struct symbol_value curr;

	/*
	 * Values for the symbol provided from outside. def[S_DEF_USER] holds
	 * the .config value.
	 */
	struct symbol_value def[S_DEF_COUNT];

	/*
	 * An upper bound on the tristate value the user can set for the symbol
	 * if it is a boolean or tristate. Calculated from prompt dependencies,
	 * which also inherit dependencies from enclosing menus, choices, and
	 * ifs. If 'n', the user value will be ignored.
	 *
	 * Symbols lacking prompts always have visibility 'n'.
	 */
	tristate visible;

	/* config entries associated with this symbol */
	struct list_head menus;

	struct list_head choice_link;

	/* SYMBOL_* flags */
	int flags;

	/* List of properties. See prop_type. */
	struct property *prop;

	/* Dependencies from enclosing menus, choices, and ifs */
	struct expr_value dir_dep;

	/* Reverse dependencies through being selected by other symbols */
	struct expr_value rev_dep;

	/*
	 * "Weak" reverse dependencies through being implied by other symbols
	 */
	struct expr_value implied;
};

*/
impl CSymbol {
    pub fn name(&self) -> Option<Cow<'_, str>> {
        unsafe {
            //println!("FOO {:#?}",self.name.as_ref().map(|obj|  CStr::from_ptr(obj).to_bytes()));

            let r = self.name
                .as_ref()
                .map(|obj| String::from_utf8_lossy(CStr::from_ptr(obj).to_bytes()));
            r
        }
    }

    pub fn get_int_value(&self) -> anyhow::Result<u64> {
        let strval = unsafe { CStr::from_ptr(self.current_value.value as *const i8) }
            .to_str()
            .context("Symbol has an invalid associated string value")?;

        use anyhow::Context;
        match self.symbol_type {
            SymbolType::Int | SymbolType::Hex | SymbolType::Unknown => {
                if strval.len() >= 2 && &strval[..2] == "0x" {
                    u64::from_str_radix(&strval[2..], 16).context("Could not parse hex value")
                } else {
                    strval.parse::<u64>().context("Could not parse integer value")
                }
            }
            _ => anyhow::bail!("Cannot get integer value for symbol of type {:?}", self.symbol_type),
        }
    }

    pub fn get_tristate_value(&self) -> Tristate {
        self.current_value.tri
    }

    pub fn symbol_type(&self) -> SymbolType {
        self.symbol_type
    }

    pub fn is_const(&self) -> bool {
        self.flags.intersects(SymbolFlags::CONST)
    }

    // pub fn is_choice(&self) -> bool {
    //     self.flags.intersects(SymbolFlags::CHOICE)
    // }
}

use bitflags::bitflags;

bitflags! {
    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct SymbolFlags: c_int {
        // WARNING might change in kernel and while unlikely should be checked
        const CONST     = 0x0001;
        const CHECK     = 0x0008;
        //const CHOICE    = 0x0010;
        //const CHOICEVAL = 0x0020;
        const VALID     = 0x0080;
       // const OPTIONAL  = 0x0100;
        const WRITE     = 0x0200;
       // const CHANGED   = 0x0400;
        const WRITTEN   = 0x0800;
       // const NOWRITE   = 0x1000;
        const CHECKED   = 0x2000;
        const WARNED    = 0x8000;
    }
}
