//! `lapp` provides a straightforward way to parse command-line
//! arguments, using the _usage text_ as a pattern.
//!
//! # Example
//! ```
//! extern crate lapp;
//!
//! let args = lapp::parse_args("
//!    A test program
//!    -v,--verbose  verbose output
//!    -k         (default 10)
//!    -s, --save (default 'out.txt')
//!    <out>      (default 'stdout')
//! ");
//! assert_eq!(args.get_bool("verbose"),false);
//! assert_eq!(args.get_integer("k"),10);
//! assert_eq!(args.get_string("save"),"out.txt");
//! assert_eq!(args.get_string("out"),"stdout");
//! ```
//!
//! The usage text or _specification_ follows these simple rules:
//! line begining with one of '-short, --long', '--long' or '-short' (flags)
//! or begining with <name> (positional arguments).
//! These may be followed by a type/default specifier (<type>) - otherwise considered a bool flag
//! with default `false`. This specifier can be a type (like '(integer)') or a default,
//! like '(default 10)`. If there's a default, the type is infered from the value - can always
//! use single quotes to insist that the flag value is a string. Otherwise this flag is
//! _required_ and must be present!
//!
//! The currently supported types are 'string','integer','bool' and 'float'. There are
//! corresponding access methods like `get_string("flag")` and so forth.
//!
//! The flag may be followed by '...' (e.g '-I... (<type>)') and it is then a _multiple_
//! flag; its value will be a vector. This vector may be empty (flag is not required).
//! If the '...' appears inside the type specifier (e.g. '-p (integer...)') then
//! the flag is expecting several space-separated values (like -p '10 20 30'); it is also
//! represented by a vector.
//!
//! Rest of line (or any other kind of line) is ignored.
//!
//! lapp scans command-line arguments using GNU-style short and long flags.
//! Short flags may be combined, and may immediately followed by a value, e.g '-vk5'.
//! As an extension, you can say '--flag=value' or '-f:value'.


use std::process;
use std::env;
use std::io;
use std::io::{Write,Read};
use std::error::Error;
use std::fs::File;
use std::fmt;

mod strutil;

#[derive(Debug)]
pub struct LappError(String);

impl fmt::Display for LappError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{}",self.0)
    }
}

impl Error for LappError {
    fn description(&self) -> &str {
        &self.0
    }
}

pub type Result<T> = std::result::Result<T,LappError>;


fn error<T, M: std::string::ToString>(msg: M) -> Result<T> {
    Err(LappError(msg.to_string()))
}


// the flag types
#[derive(Debug, PartialEq)]
enum Type {
    Str,
    Int,
    Float,
    Bool,
    FileIn,
    FileOut,
    None,
    Arr(Box<Type>)
}

impl Default for Type {
    fn default() -> Type { Type::None }
}


impl Type {
    fn from_name(s: &str) -> Result<Type> {
        match s {
        "string" => Ok(Type::Str),
        "integer" => Ok(Type::Int),
        "float" => Ok(Type::Float),
        "bool" => Ok(Type::Bool),
        "infile" => Ok(Type::FileIn),
        "outfile" => Ok(Type::FileOut),
        _ => error(format!("not a known type {}",s))
        }
    }

    fn array_type(&self) -> Option<&Type> {
        // &**bt means (from R to L) deref bt, unbox type, return reference
        match *self {Type::Arr(ref bt) => Some(&**bt), _ => None}
    }

    fn create_empty_array(self) -> Type {
        Type::Arr(Box::new(self))
    }

    fn short_name(&self) -> String {
        (match *self {
         Type::Str => "string",
         Type::Int => "integer",
         Type::Float => "float",
         Type::Bool => "bool",
         Type::FileIn => "infile",
         Type::FileOut => "outfile",
         Type::Arr(_) => "array",
         _ => "bad"
        }).to_string()
    }

    fn rust_name(&self, multiple: bool) -> String {
        let mut res = match *self {
            Type::Bool => "bool".into(),
            Type::Float => "f32".into(),
            Type::Int => "i32".into(),
            Type::Str => "String".into(),
            Type::FileIn => "Box<Read>".into(),
            Type::FileOut => "Box<Write>".into(),
            Type::Arr(ref t) => format!("Vec<{}>",t.rust_name(false)),
            _ => "bad".into()
        };
        if multiple {
            res = format!("Vec<{}>",res);
        }
        res
    }


    fn parse_string(&self, s: &str) -> Result<Value> {
        match *self {
        Type::Str => Ok(Value::Str(s.to_string())),
        Type::Int =>
            match s.parse::<i32>() {
                Ok(n) => Ok(Value::Int(n)),
                Err(e) => error(format!("can't convert '{}' to integer - {}",s,e.description()))
            },
        Type::Float =>
            match s.parse::<f32>() {
                Ok(v) => Ok(Value::Float(v)),
                Err(e) => error(format!("can't convert '{}' to float - {}",s,e.description()))
            },
        Type::FileIn => Ok(Value::FileIn(s.to_string())),
        Type::FileOut => Ok(Value::FileOut(s.to_string())),
        Type::Arr(ref bt) => {
            // multiple values either space or comma separated
            let parts: Vec<_> = if s.find(',').is_some() {
                s.split(',').collect()
            } else {
                s.split_whitespace().collect()
            };
            let mut res = Vec::new();
            for part in parts {
                let v = bt.parse_string(part)?;
                res.push(Box::new(v));
            }
            Ok(Value::Arr(res))
          }
        _ => error(format!("can't convert '{}' to {:?}",s,self))
        }
    }

}

// and values...
#[derive(Debug, Clone)]
enum Value {
    Str(String),
    Int(i32),
    Float(f32),
    Bool(bool),
    FileIn(String),
    FileOut(String),
    None,
    Arr(Vec<Box<Value>>)
}

impl Default for Value {
    fn default() -> Value { Value::None }
}


impl Value {
    fn as_string(&self) -> Result<String> {
        match *self { Value::Str(ref s) => Ok(s.clone()), _ => error("not a string") }
    }

    fn as_int(&self) -> Result<i32> {
        match *self { Value::Int(n) => Ok(n), _ => error("not an integer" )}
    }

    fn as_float(&self) -> Result<f32> {
        match *self { Value::Float(x) => Ok(x), _ => error("not a float") }
    }

    fn as_bool(&self) -> Result<bool> {
        match *self { Value::Bool(b) => Ok(b), _ => error("not a boolean") }
    }

    fn as_filein(&self) -> Result<Box<Read>> {
        match *self {
             Value::FileIn(ref s) => {
                if s == "stdin" { return Ok(Box::new(io::stdin())); }
                match File::open(s) {
                    Ok(f) => Ok(Box::new(f)),
                    Err(e) => error(format!("can't open '{}' for reading: {}",s, e.description()))
                }
             },
              _ => error("not a infile")
        }
    }

    fn as_fileout(&self) -> Result<Box<Write>> {
        match *self {
             Value::FileOut(ref s) => {
                if s == "stdout" { return Ok(Box::new(io::stdout())); }
                match File::create(s) {
                    Ok(f) => Ok(Box::new(f)),
                    Err(e) => error(format!("can't open '{}' for writing: {}",s, e.description()))
                }
             },
              _ => error("not an outfile")
        }
    }


    fn as_array(&self) -> Result<&Vec<Box<Value>>> {
        match *self { Value::Arr(ref vi) => Ok(vi), _ => error("not an array") }
    }

    fn type_of(&self) -> Type {
        match *self {
        Value::Str(_) => Type::Str,
        Value::Int(_) => Type::Int,
        Value::Float(_) => Type::Float,
        Value::Bool(_) => Type::Bool,
        Value::FileIn(_) => Type::FileIn,
        Value::FileOut(_) => Type::FileOut,
        Value::None => Type::None,
        // watch out here...
        Value::Arr(ref v) => (*v[0]).type_of()
        }
    }

    fn from_value (val: &str) -> Result<Value> {
        let firstc = val.chars().next().unwrap();
        if firstc.is_digit(10) {
            let t = if val.find('.').is_some() { Type::Float } else { Type::Int };
            t.parse_string(val)
        } else
        if firstc == '\'' { // strip quotes
            Ok(Value::Str((&val[1..(val.len()-1)]).to_string()))
        } else
        if val == "stdin" {
            Ok(Value::FileIn("stdin".into()))
        } else
        if val == "stdout" {
            Ok(Value::FileOut("stdout".into()))
        } else {
            Ok(Value::Str(val.to_string()))
        }
    }

    fn empty_array() -> Value {
        let empty: Vec<Box<Value>> = Vec::new();
        Value::Arr(empty)
    }

}

#[derive(Default)]
struct Flag {
    long: String,
    short: char,
    vtype: Type,
    value: Value,
    defval: Value,
    is_set: bool,
    is_multiple: bool,
    pos: usize,
    help: String,
    constraint: Option<Box< Fn(Value) -> Result<Value> >>,
}

impl Flag {
    fn set_value_from_string(&mut self, arg: &str) -> Result<()> {
        let mut v = self.vtype.parse_string(arg)?;
        // there may be a constrait on this flag value
        if let Some(ref constraint) = self.constraint {
            v = constraint(v)?;
        }
        self.set_value(v)?;
        Ok(())
    }

    fn set_range_constraint(&mut self, b1: &str, b2: &str) -> Result<()> {
        let b1 = Value::from_value(b1)?;
        let b2 = Value::from_value(b2)?;
        if b1.type_of() != b2.type_of() {
            return error("range values must be same type");
        }
        let tn = b1.type_of().short_name();
        if ! (tn == "integer" || tn == "float") {
            return error("range values must be integer or float");
        }
        self.vtype = b1.type_of();

        if tn == "integer" {
            let i1 = b1.as_int().unwrap();
            let i2 = b2.as_int().unwrap();
            let msg = format!("{} {}..{}",self.long,i1,i2);
            self.constraint = Some(Box::new(
                move |v| {
                    let i = v.as_int().unwrap();
                    if i < i1 || i > i2 {
                        return error(format!("{} out of range",msg));
                    }
                    Ok(Value::Int(i))
                }
            ));
        } else {
            let x1 = b1.as_float().unwrap();
            let x2 = b2.as_float().unwrap();
            let msg = format!("{} {}..{}",self.long,x1,x2);
            self.constraint = Some(Box::new(
                move |v| {
                    let x = v.as_float().unwrap();
                    if x < x1 || x > x2 {
                        return error(format!("{} out of range",msg));
                    }
                    Ok(Value::Float(x))
                }
            ));
        }
        Ok(())
    }

    fn set_value(&mut self, v: Value) -> Result<()> {
        if self.is_set && ! self.is_multiple {
            return error(format!("flag already specified {}",self.long));
        }
        self.is_set = true;
        if ! self.is_multiple {
            self.value = v;
        } else {
            if let Value::Arr(ref mut arr) = self.value {
                arr.push(Box::new(v));
            }
        }
        Ok(())
    }

    // When checking any missing flags after scanning args, insist
    // that they have default values - otherwise they are 'required'.
    // (Array values may be empty tho)
    fn check(&mut self) -> Result<()> {
        if ! self.is_set {
            if let Value::None = self.defval  {
                if let Type::Arr(_) = self.vtype {
                } else
                if ! self.is_multiple {
                    return error(format!("required flag {}",self.long));
                }
            } else {
                self.value = self.defval.clone();
            }
        }
        Ok(())
    }

    fn rust_name(&self) -> String {
        // long name may need massaging to become a Rust variable name
        // The result must be snake_case to keep compiler happy!
        let mut name = self.long.replace('-',"_").to_lowercase().to_string();
        let firstc = name.chars().nth(0).unwrap();
        if firstc.is_digit(10) || firstc == '_' {
            name = format!("c_{}",name);
        }
        name
    }

    fn rust_type(&self) -> String {
        self.vtype.rust_name(self.is_multiple)
    }

    fn getter_name(&self) -> String {
        let mut tname = self.vtype.short_name();
        // Is this an array flag? Two possibilities - the type is an array,
        // or our multiple flag is set.
        let maybe_array = self.vtype.array_type();
        if maybe_array.is_some() {
            tname = maybe_array.unwrap().short_name() + "s";
        } else
        if self.is_multiple {
            tname.push('s');
        }
        format!("args.get_{}(\"{}\")",tname,self.long)
    }


}

pub struct Args<'a> {
    flags: Vec<Flag>,
    pos: usize,
    text: &'a str,
    varargs: bool,
}


impl <'a> Args<'a> {
    /// provide a _usage string_ from which we extract flag definitions
    pub fn new(text: &'a str) -> Args {
        Args{flags: Vec::new(), pos: 0, text: text, varargs: false}
    }

    /// bail out of program with non-zero return code.
    /// May force this to panic instead with the
    /// EASY_DONT_QUIT_PANIC environment variable.
    pub fn quit(&self, msg: &str) -> ! {
        let path = env::current_exe().unwrap();
        let exe = path.file_name().unwrap();
        let text = format!("{:?} error: {}",exe,msg);
        if env::var("RUST_BACKTRACE").is_ok() {
            panic!(text);
        } else {
            writeln!(&mut io::stderr(),"{}",text).unwrap();
            process::exit(1);
        }
    }

    /// create suggested variable or struct declarations for accessing the flags...
    pub fn declarations(&mut self, struct_name: &str) -> String {
        if let Err(e) = self.parse_spec() {
            self.quit(e.description());
        }
        let mut res = String::new();
        if struct_name.len() > 0 {
            res += &format!("const USAGE: &'static str = \"\n{}\";\n",self.text);
            res += &format!("#[derive(Debug)]\nstruct {} {{\n",struct_name);
            for f in &self.flags {
                res += &format!("\t{}: {},\n",f.rust_name(),f.rust_type());
            }
            res += &format!(
                "}}\n\nimpl {} {{\n\tfn new() -> ({},lapp::Args<'static>) {{\n",
                struct_name,struct_name);
            res += &format!(
                "\t\tlet args = lapp::parse_args(USAGE);\n\t\t({}{{\n",struct_name);
            for f in &self.flags {
                res += &format!("\t\t\t{}: {},\n",f.rust_name(),f.getter_name());
            }
            res += &format!("\t\t}},args)\n\t}}\n}}\n\n");
        } else {
            for f in &self.flags {
                res += &format!("    let {} = {};\n",
                    f.rust_name(),f.getter_name());
            }
        }
        res
    }

    pub fn dump(&mut self) {
        self.parse();
        for f in &self.flags {
            println!("flag '{}' value {:?}",f.long,f.value);
        }
    }

    fn parse(&mut self) {
        if let Err(e) = self.parse_spec() { self.quit(e.description()); }
        let v: Vec<String> = env::args().skip(1).collect();
        if let Err(e) = self.parse_command_line(v) { self.quit(e.description()); }
    }

    fn parse_spec(&mut self) -> Result<()> {
        for line in self.text.lines() {
            self.parse_spec_line(line)?;
        }
        if let Err(_) = self.flags_by_long("help") {
//            self.parse_spec_line("   -h,--help this help").unwrap();
        }
        Ok(())
    }


    fn parse_spec_line(&mut self, mut slice: &str) -> Result<()> {
        use strutil::*;

        if let Some(idx) = slice.find(|c: char| ! c.is_whitespace()) {
            let mut flag: Flag = Default::default();
            let mut is_positional = false;
            slice = &slice[idx..];
            let is_flag = starts_with(&mut slice,"-");
            let mut long_flag = starts_with(&mut slice,"-");
            if is_flag && ! long_flag { // short flag
                flag.short = (&slice[0..1]).chars().next().unwrap();
                if ! flag.short.is_alphanumeric() {
                    return error(format!("{:?} isn't allowed: only letters or digits in short flags",flag.short));
                }
                slice = &slice[1..];
                if let Some(0) = slice.find(|c: char| c.is_alphanumeric()) {
                   return error(format!("short flags should have one character"));
                }
                if starts_with(&mut slice,",") {
                    slice = skipws(slice);
                    long_flag = starts_with(&mut slice,"--");
                    if ! long_flag {
                        return error("expecting long flag after short flag");
                    }
                }
            }
            if long_flag {
                let idx = slice.find(|c: char| ! (c.is_alphabetic() || c == '_' || c == '-')).unwrap();
                let parts = slice.split_at(idx);
                flag.long = parts.0.to_string();
                slice = parts.1;
                if parts.1.starts_with(",") {
                    slice = &slice[1..];
                }
                // TBD: enforce char rules for long names
            } else
            if starts_with(&mut slice, "<") { // positional argument
                flag.long = grab_upto(&mut slice, ">")?;
                self.pos = self.pos + 1;
                flag.pos = self.pos;
                is_positional = true;
            }
            if flag.long == "" && flag.short == '\0' {
                // not a significant line, ignore!
                return Ok(());
            }
            if flag.long == "" { // just a short flag
                flag.long = flag.short.to_string();
            }
            slice = skipws(slice);
            if starts_with(&mut slice,"...") {
                flag.is_multiple = true;
                slice = skipws(slice);
            }
            if starts_with(&mut slice,"(") {
                let r = grab_upto(&mut slice, ")")?;
                let mut rest = r.as_str();
                let multable = ends_with(&mut rest,"...");
                if let Some((b1,b2)) = split_with(rest,"..") {
                    // bounds on a number type
                    flag.set_range_constraint(b1,b2)?;
                } else {
                    // default VALUE or TYPE
                    let parts: Vec<_> = rest.split_whitespace().collect();
                    if parts.len() == 0 {
                        return error(format!("nothing inside type specifier"));
                    }
                    if parts.len() == 2 {
                        if parts[0] == "default" {
                            flag.defval = Value::from_value(parts[1])?;
                            flag.vtype = flag.defval.type_of();
                        } else {
                            return error(format!("expecting (default <value>)"));
                        }
                    } else {
                        flag.vtype = Type::from_name(parts[0])?;
                    }
                }
                // if type is followed by '...' then the flag is also represented
                // by a vector (e.g. --ports '8080 8081 8082').
                // UNLESS it is a positional argument,
                // where it is considered multiple!
                if multable {
                    flag.defval = Value::empty_array();
                    if is_positional {
                        flag.is_multiple = true;
                        if self.varargs {
                            return error("only last argument can occur multiple times");
                        }
                        self.varargs = true;
                    } else { // i.e the flag type is an array of a basic scalar type
                        flag.vtype = flag.vtype.create_empty_array();
                    }
                }
                if flag.is_multiple {
                    flag.value = Value::empty_array();
                }
            } else {
                flag.defval = Value::Bool(false);
                flag.vtype = Type::Bool;
            }
            if slice.len() > 0 {
                flag.help = skipws(slice).to_string();
            }

            // it is an error to specify a flag twice...
            if self.flags_by_long_ref(&flag.long).is_ok() {
                return error(format!("flag {:?} already defined",flag.long));
            }
            self.flags.push(flag);
        }
        Ok(())

    }

    fn flags_by_long(&mut self, s: &str) -> Result<&mut Flag> {
        self.flags.iter_mut()
            .filter(|&ref f| f.long == s)
            .next().ok_or(LappError(format!("no long flag '{}'",s)))
    }

    fn flags_by_long_ref(&self, s: &str) -> Result<&Flag> {
        self.flags.iter()
            .filter(|&f| f.long == s)
            .next().ok_or(LappError(format!("no long flag '{}'",s)))
    }

    fn flags_by_short(&mut self, ch: char) -> Result<&mut Flag> {
        self.flags.iter_mut()
            .filter(|&ref f| f.short == ch)
            .next().ok_or(LappError(format!("no short flag '{}'",ch)))
    }

    fn flags_by_pos(&mut self, pos: usize) -> Result<&mut Flag> {
        self.flags.iter_mut()
            .filter(|&ref f| f.pos == pos)
            .next().ok_or(LappError(format!("no positional arg '{}'",pos)))
    }

    fn parse_command_line(&mut self, v: Vec<String>) -> Result<()> {
        use strutil::*;
        let mut iter = v.into_iter();

        fn nextarg(name: &str, ms: Option<String>) -> Result<String> {
            if  ms.is_none() {return error(format!("no value for flag '{}'",name));}
            Ok(ms.unwrap())
        };

        // flags _may_ have the value after a = or : delimiter
        fn extract_flag_value(s: &mut &str) -> String {
            if let Some(idx) = s.find(|c: char| c == '=' || c == ':') {
               *s = &s[0..idx];
               &s[idx+1..]
            } else {
               ""
           }.to_string()
        }

        let mut parsing = true;
        let mut k = 1;
        while let Some(arg) = iter.next() {
            let mut s = arg.as_str();
             if parsing && starts_with(&mut s, "--") { // short flag
                if s.len() == 0 { // plain '--' means 'stop arg processing'
                    parsing = false;
                } else {
                    let mut rest = extract_flag_value(&mut s);
                    let mut flag = self.flags_by_long(s)?;
                    if flag.vtype != Type::Bool { // then it needs a value....
                        if rest == "" {  // try grab the next arg
                            rest = nextarg(s,iter.next())?;
                        }
                        flag.set_value_from_string(&rest)?;
                    } else {
                        flag.set_value(Value::Bool(true))?;
                    }
                }
            } else
            if parsing && starts_with(&mut s,"-") { // short flag
                // there can be multiple short flags
                // although only the last one can take a value
                let mut chars = s.chars();
                let mut idx = 0;
                for ch in chars.by_ref() {
                    let mut flag = self.flags_by_short(ch)?;
                    if flag.vtype != Type::Bool {
                        let s = s.clone();
                        let mut rest = (&s[idx+1..]).to_string();
                        if rest == "" {
                            rest = nextarg(&flag.long,iter.next())?;
                        }
                        flag.set_value_from_string(&rest)?;
                        break;
                    } else {
                       flag.set_value(Value::Bool(true))?;
                    }
                    idx += 1;
                }
            } else {  // positional argument
                let mut flag = self.flags_by_pos(k)?;
                flag.set_value_from_string(s)?;
                // multiple arguments are added to the vector value
                if ! flag.is_multiple {
                    k += 1;
                }

            }
        }


        if self.flags_by_long_ref("help").is_ok() {
            println!("{}",self.text);
            process::exit(0);
        }


        // fill in defaults. If a default isn't available it's
        // a required flag. If not specified the flag value is set to an error
        for flag in &mut self.flags {
            flag.check()?;
        }
        Ok(())
    }

    fn get_flag_value (&self, name: &str) -> &Value {
        if let Ok(ref flag) = self.flags_by_long_ref(name) {
           &flag.value
        } else {
            self.quit(&format!("unknown flag '{}'",name))
        }
    }

    fn bad_flag (&self, tname: &str, msg: &str) -> ! {
        self.quit(&format!("flag '{}': {}",tname,msg))
    }

    fn get_flag<T, F: Fn(&Value) -> Result<T>> (&self, name: &str, extract: F) -> T {
        match extract(self.get_flag_value(name)) {
            Ok(v) => v,
            Err(e) => self.bad_flag(name,e.description())
        }
    }

    /// get flag as a string, quitting otherwise.
    pub fn get_string(&self, name: &str) -> String {
        self.get_flag(name,|v| v.as_string())
    }

    /// get flag as an integer, quitting otherwise.
    pub fn get_integer(&self, name: &str) -> i32 {
        self.get_flag(name,|v| v.as_int())
    }

    /// get flag as a float, quitting otherwise.
    pub fn get_float(&self, name: &str) -> f32 {
        self.get_flag(name,|v| v.as_float())
    }

    /// get flag as a bool, quitting otherwise.
    pub fn get_bool(&self, name: &str) -> bool {
        self.get_flag(name,|v| v.as_bool())
    }

    /// get flag as a file for reading, quitting otherwise.
    pub fn get_infile(&self, name: &str) -> Box<Read> {
        self.get_flag(name,|v| v.as_filein())
    }

    /// get flag as a file for writing, quitting otherwise.
    pub fn get_outfile(&self, name: &str) -> Box<Write> {
        self.get_flag(name,|v| v.as_fileout())
    }

    fn get_boxed_array(&self, name: &str, kind: &str) -> &Vec<Box<Value>> {
        match self.get_flag_value(name).as_array() {
            Err(e) => self.bad_flag(name,e.description()),
            Ok(arr) => {
                // empty array matches all types
                if arr.len() == 0 { return arr; }
                // otherwise check the type of the first element
                let ref v = *(arr[0]);
                let tname = v.type_of().short_name();
                if tname == kind {arr} else  { self.bad_flag(name,kind) }
            }
        }
    }


    fn get_array<T,F>(&self, name: &str, kind: &str, extract: F) -> Vec<T>
    where T: Sized, F: Fn(&Box<Value>)->Result<T> {
        let va = self.get_boxed_array(name,kind);
        let mut res = Vec::new();
        for v in va {
            let n = extract(v).unwrap();
            res.push(n);
        }
        res
    }

    /// get multiple flag as an array of strings, quitting otherwise.
    pub fn get_strings(&self, name: &str) -> Vec<String> {
        self.get_array(name,"string",|b| b.as_string())
    }

    /// get multiple flag as an array of integers, quitting otherwise.
    pub fn get_integers(&self, name: &str) -> Vec<i32> {
        self.get_array(name,"integer",|b| b.as_int())
    }

    /// get multiple flag as an array of floats, quitting otherwise.
    pub fn get_floats(&self, name: &str) -> Vec<f32> {
        self.get_array(name,"float",|b| b.as_float())
    }

}

/// parse the command-line specification and use it
/// to parse the program's command line args.
/// As before, quits on any error.
pub fn parse_args(s: &str) -> Args {
    let mut res = Args::new(s);
    res.parse();
    res
}


#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE: &'static str = "
Testing Lapp
  -v, --verbose verbose flag
  -k   arb flag
  -o, --output (default 'stdout')
  -p   (integer...)
  -I, --include... (string)
  <in> (string)
  <out> (string...)
";

    fn arg_strings(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    fn empty_strings() -> Vec<String> {
        Vec::new()
    }

    struct SimpleTest {
        verbose: bool,
        k: bool,
        output: String,
        p: Vec<i32>,
        include: Vec<String>,
        out: Vec<String>
    }

    impl SimpleTest {
        fn new(test_args: &[&str]) -> SimpleTest {
            let mut args = Args::new(SIMPLE);
            args.parse_spec().expect("spec failed");
            args.parse_command_line(arg_strings(test_args)).expect("scan failed");
            SimpleTest {
                verbose: args.get_bool("verbose"),
                k: args.get_bool("k"),
                output: args.get_string("output"),
                p: args.get_integers("p"),
                include: args.get_strings("include"),
                out: args.get_strings("out")
            }
        }
    }

    #[test]
    fn test_simple_just_out() {
        let res = SimpleTest::new(&["boo","hello"]);
        assert_eq!(res.verbose,false);
        assert_eq!(res.k,false);
        assert_eq!(res.output,"stdout");
        assert_eq!(res.p,&[]);
        assert_eq!(res.out,&["hello"]);
    }

    #[test]
    fn test_simple_bool_flags() {
        let res = SimpleTest::new(&["boo","-vk","hello"]);
        assert_eq!(res.verbose,true);
        assert_eq!(res.k,true);
        assert_eq!(res.output,"stdout");
        assert_eq!(res.p,&[]);
        assert_eq!(res.out,&["hello"]);
    }

    #[test]
    fn test_simple_array_flag() {
        let res = SimpleTest::new(&["boo","-p","10 20 30","hello"]);
        assert_eq!(res.verbose,false);
        assert_eq!(res.k,false);
        assert_eq!(res.output,"stdout");
        assert_eq!(res.p,&[10,20,30]);
        assert_eq!(res.out,&["hello"]);
    }

    #[test]
    fn test_simple_multiple_positional_args() {
        let res = SimpleTest::new(&["boo","hello","baggins","--","--frodo"]);
        assert_eq!(res.verbose,false);
        assert_eq!(res.k,false);
        assert_eq!(res.output,"stdout");
        assert_eq!(res.p,&[]);
        assert_eq!(res.include,empty_strings());
        assert_eq!(res.out,&["hello","baggins","--frodo"]);
    }

    #[test]
    fn test_simple_multiple_flags() {
        let res = SimpleTest::new(&["boo","-I.","-I..","--include","lib","hello"]);
        assert_eq!(res.verbose,false);
        assert_eq!(res.k,false);
        assert_eq!(res.output,"stdout");
        assert_eq!(res.p,&[]);
        assert_eq!(res.include,&[".","..","lib"]);
        assert_eq!(res.out,&["hello"]);
    }


}
