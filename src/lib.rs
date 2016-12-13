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


extern crate scanlex;
use scanlex::{Scanner,Token,ScanError};
use std::process;
use std::env;
use std::io;
use std::io::Write;
use std::error::Error;

fn error<T>(msg: String) -> Result<T,ScanError> {
    Err(ScanError::new(&msg))
}

// the flag types
#[derive(Debug)]
#[derive(PartialEq)]
enum Type {
    Str,
    Int,
    Float,
    Bool,
    None,
    Error,
    Arr(Box<Type>)
}

impl Type {
    fn from_name(s: &str) -> Result<Type,ScanError> {
        match s {
        "string" => Ok(Type::Str),
        "integer" => Ok(Type::Int),
        "float" => Ok(Type::Float),
        "bool" => Ok(Type::Bool),
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
         Type::Arr(_) => "array",
         _ => "bad"
        }).to_string()
    }

    fn rust_name(&self, multiple: bool) -> String {
        let mut res = match *self {
            Type::Bool => "bool".to_string(),
            Type::Float => "f32".to_string(),
            Type::Int => "i32".to_string(),
            Type::Str => "String".to_string(),
            Type::Arr(ref t) => format!("Vec<{}>",t.rust_name(false)),
            _ => "bad".to_string()
        };
        if multiple {
            res = format!("Vec<{}>",res);
        }
        res
    }


    fn parse_string(&self, s: &str) -> Value {
        match *self {
        Type::Str => Value::Str(s.to_string()),
        Type::Int =>
            match s.parse::<i32>() {
            Ok(n) => Value::Int(n),
            Err(e) => Value::Error(format!("can't convert '{}' to integer - {}",s,e.description()))
            },
        Type::Float =>
            match s.parse::<f32>() {
            Ok(v) => Value::Float(v),
            Err(e) => Value::Error(format!("can't convert '{}' to float - {}",s,e.description()))
            },
        Type::Arr(ref bt) => {
            let parts: Vec<_> = if s.find(',').is_some() {
                s.split(',').collect()
            } else {
                s.split_whitespace().collect()
            };
            let mut res = Vec::new();
            for part in parts {
                let v = (*bt).parse_string(part);
                if v.is_err() { return v; }
                res.push(Box::new(v));
            }
            Value::Arr(res)
          }
        _ => Value::Error(format!("can't convert '{}' to {:?}",s,self))
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
    None,
    Error(String),
    Arr(Vec<Box<Value>>)
}

impl Value {
    fn as_string(&self) -> Option<String> {
        match *self { Value::Str(ref s) => Some(s.clone()), _ => None }
    }

    fn as_int(&self) -> Option<i32> {
        match *self { Value::Int(n) => Some(n), _ => None }
    }

    fn as_float(&self) -> Option<f32> {
        match *self { Value::Float(x) => Some(x), _ => None }
    }

    fn as_bool(&self) -> Option<bool> {
        match *self { Value::Bool(b) => Some(b), _ => None }
    }

    fn as_array(&self) -> Option<&Vec<Box<Value>>> {
        match *self { Value::Arr(ref vi) => Some(vi), _ => None }
    }

    fn type_of(&self) -> Type {
        match *self {
        Value::Str(_) => Type::Str,
        Value::Int(_) => Type::Int,
        Value::Float(_) => Type::Float,
        Value::Bool(_) => Type::Bool,
        Value::None => Type::None,
        Value::Error(_) => Type::Error,
        // watch out here...
        Value::Arr(ref v) => (*v[0]).type_of()
        }
    }

    fn is_err(&self) -> bool {
        if let Value::Error(_) = *self {true} else {false}
    }

    fn from_value (tok: &Token) -> Result<Value,ScanError> {
        match *tok {
        Token::Str(ref s) => Ok(Value::Str(s.clone())),
        Token::Num(x) => if x.floor()==x {
                Ok(Value::Int(x as i32))
            } else {
                Ok(Value::Float(x as f32))
            },
        Token::Iden(ref s) => Ok(Value::Str(s.clone())),
        Token::Char(ch) => Ok(Value::Str(ch.to_string())),
        _ => error(format!("bad default value {:?}",tok))
        }
    }

    fn empty_array() -> Value {
        let empty: Vec<Box<Value>> = Vec::new();
        Value::Arr(empty)
    }

}

#[derive(Debug)]
struct Flag {
    long: String,
    short: char,
    vtype: Type,
    value: Value,
    defval: Value,
    is_set: bool,
    is_multiple: bool,
    pos: usize
}

impl Flag {
    fn set_value_from_string(&mut self, arg: &str) {
        // might fail, but we have Value::Error...
        let v = self.vtype.parse_string(arg);
        self.set_value(v);
    }

    fn set_value(&mut self, v: Value) {
        if self.is_set && ! self.is_multiple {
            self.value = Value::Error(format!("flag already specified {}",self.long));
            return;
        }

        self.is_set = true;
        if ! self.is_multiple {
            self.value = v;
        } else {
            if let Value::Arr(ref mut arr) = self.value {
                arr.push(Box::new(v));
            }
        }
    }

    // When checking any missing flags after scanning args, insist
    // that they have default values - otherwise they are 'required'.
    // (Array values may be empty tho)
    fn check(&mut self) {
        if ! self.is_set {
            if let Value::None = self.defval  {
                if let Type::Arr(_) = self.vtype {
                } else
                if ! self.is_multiple {
                    self.value = Value::Error(format!("required flag {}",self.long));
                }
            } else {
                self.value = self.defval.clone();
            }
        }
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
    text: &'a str
}

impl <'a> Args<'a> {
    /// provide a _usage string_ from which we extract flag definitions
    pub fn new(text: &'a str) -> Args {
        Args{flags: Vec::new(), pos: 0, text: text}
    }

    /// bail out of program with non-zero return code.
    /// May force this to panic instead with the
    /// EASY_DONT_QUIT_PANIC environment variable.
    pub fn quit(&self, msg: &str) -> ! {
        let path = env::current_exe().unwrap();
        let exe = path.file_name().unwrap();
        let text = format!("{:?} error: {}",exe,msg);
        if env::var("EASY_DONT_QUIT_PANIC").is_ok() {
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
        let v: Vec<String> = std::env::args().skip(1).collect();
        if let Err(e) = self.parse_command_line(v) { self.quit(e.description()); }
    }

    fn parse_spec(&mut self) -> Result<(),ScanError> {
        for line in self.text.lines() {
            if let Err(e) = self.parse_spec_line(line) {
                return Err(e);
            }
        }
        if let Err(_) = self.flags_by_long("help") {
            self.parse_spec_line("   -h,--help this help").unwrap();
        }
        Ok(())
    }


    fn parse_spec_line(&mut self, line: &str) -> Result<(),ScanError> {
        let mut scan = Scanner::new(line);
        if ! scan.skip_whitespace() { return Ok(()); } // empty!

        let mut ch = scan.nextch();
        let mut long = String::new();
        let mut short = '\0';
        let mut is_multiple = false;
        let mut is_positional = false;
        if ch == '-' { // this line defines a flag
            if scan.peek() != '-' { // short flag name
                short = scan.nextch();
                ch = scan.nextch();
                if ! short.is_alphanumeric() {
                    return error(format!("{:?} isn't allowed: only letters or digits in short flags",short));
                }
                if ch != ',' && ch != ' ' {
                    return error(format!("{:?} isn't allowed: short flag not followed by comma or space",short));
                }
                if ch == ',' { // expecting long flag!
                    if scan.peek() == ' ' {
                        scan.nextch();
                    }
                    ch = scan.nextch();
                    if ch != '-' {
                        return error(format!("expected long flag after short flag {:?}",short));
                    }
                }
            }
            if ch == '-' { // long flag nane
                scan.nextch(); // skip -
                long = scan.grab_while(|c| c.is_alphanumeric() || c == '-' || c == '_');
                if scan.peek() != '.' && scan.peek() != ' ' {
                    return error(format!("{:?} isn't allowed: long flag chars are alphanumeric, '_' or '-'",scan.peek()));
                }
            }
            if long == "" {
                long = short.to_string();
            }
            // flag followed by '...' means that the flag may
            // occur multiple times, with the results stored in a vector.
            // For instance '-I. -Ilib'
            is_multiple = scan.peek() == '.';
            if is_multiple {
                scan.skip_until(|c| c != '.');
            }
        } else
        if ch == '<' { // positional argument
            long = scan.take_until(&['>']);
            short = '\0';
            self.pos = self.pos + 1;
            scan.nextch();
            is_positional = true;
        } else { // just non-meaningful comments...
            return Ok(());
        }

        // May be followed by type/default in ()
        scan.skip_whitespace();
        ch = scan.nextch();
        let mut flag_type = Type::Bool;
        let mut default_val = Value::None;
        let mut flag_value = Value::None;
        if ch == '(' {
            let atype = try!(scan.get_iden());
            if atype == "default" { // followed by the default value
                let next = scan.get();
                default_val = try!(Value::from_value(&next));
                // with type deduced from the default
                flag_type = default_val.type_of();
            } else {
                flag_type = try!(Type::from_name(&atype));
            }
            // if type is followed by '...' then the flag is also represented
            // by a vector (e.g. --ports '8080 8081 8082').
            // UNLESS it is a positional argument,
            // where it is considered multiple!
            if scan.peek() == '.' {
                default_val = Value::empty_array();
                if is_positional {
                    is_multiple = true;
                } else { // i.e the flag type is an array of a basic scalar type
                    flag_type = flag_type.create_empty_array();
                }
            }
            if is_multiple {
                flag_value = Value::empty_array();
            }
        } else {
            default_val = Value::Bool(false);
        }

        // it is an error to specify a flag twice...
        if self.flags_by_long_ref(&long).is_ok() {
            return error(format!("flag {} already defined",long));
        }

        let flag = Flag{long: long, short: short,
            vtype: flag_type, value: flag_value,
            defval: default_val,
            is_set: false, pos: self.pos,
            is_multiple: is_multiple
        };
        self.flags.push(flag);
        Ok(())

    }

    fn flags_by_long(&mut self, s: &str) -> Result<&mut Flag,ScanError> {
        self.flags.iter_mut()
            .filter(|&ref f| f.long == s)
            .next().ok_or(ScanError::new(&format!("no long flag '{}'",s)))
    }

    fn flags_by_long_ref(&self, s: &str) -> Result<&Flag,ScanError> {
        self.flags.iter()
            .filter(|&f| f.long == s)
            .next().ok_or(ScanError::new(&format!("no long flag '{}'",s)))
    }

    fn flags_by_short(&mut self, ch: char) -> Result<&mut Flag,ScanError> {
        self.flags.iter_mut()
            .filter(|&ref f| f.short == ch)
            .next().ok_or(ScanError::new(&format!("no short flag '{}'",ch)))
    }

    fn flags_by_pos(&mut self, pos: usize) -> Result<&mut Flag,ScanError> {
        self.flags.iter_mut()
            .filter(|&ref f| f.pos == pos)
            .next().ok_or(ScanError::new(&format!("no positional arg '{}'",pos)))
    }

    fn parse_command_line(&mut self, v: Vec<String>) -> Result<(),ScanError> {
        let mut iter = v.into_iter();

        fn nextarg(name: &str, ms: Option<String>) -> Result<String,ScanError> {
            if  ms.is_none() {return error(format!("no value for flag '{}'",name));}
            Ok(ms.unwrap())
        };

        let mut parsing = true;
        let mut k = 1;
        while let Some(s) = iter.next() {
            let mut scan = Scanner::new(&s);
            let mut ch = scan.nextch();
            if ch == '-' && parsing {
                if scan.peek() == '-' { // long flag
                    scan.nextch();
                    if scan.peek() != '\0' {
                        // flag may immediately followed by its value, with optional = or :
                        let long = scan.take_until(&['=',':']);
                        scan.nextch();
                        let mut rest = scan.take_rest();
                        let mut flag = try!(self.flags_by_long(&long));
                        if flag.vtype != Type::Bool {
                            if rest == "" {  // otherwise try grab the next arg
                                rest = try!(nextarg(&long,iter.next()));
                            }
                            flag.set_value_from_string(&rest);
                        } else {
                            flag.set_value(Value::Bool(true));
                        }
                    } else { // just '--' switches off argument parsing
                        parsing = false;
                    }
                } else {
                    loop {
                        // similar; there can be multiple short flags
                        // although only the last one can take a value
                        ch = scan.nextch();
                        if ch == '\0' { break; }
                        let mut flag = try!(self.flags_by_short(ch));
                        if flag.vtype != Type::Bool {
                            ch = scan.peek();
                            if ch == '=' || ch == ':' { scan.nextch(); }
                            let mut rest = scan.take_rest();
                            if rest == "" {
                                rest = try!(nextarg(&flag.long,iter.next()));
                            }
                            flag.set_value_from_string(&rest);
                        } else {
                            flag.set_value(Value::Bool(true));
                        }
                    }
                }
            } else { // positional argument
                let mut flag = try!(self.flags_by_pos(k));
                if ! flag.is_multiple {
                    k += 1;
                }
                flag.set_value_from_string(&s);
            }
        }

        if self.get_flag_value("help").as_bool().is_some() {
            println!("{}",self.text);
            process::exit(0);
        }


        // fill in defaults. If a default isn't available it's
        // a required flag. If not specified the flag value is set to an error
        for flag in &mut self.flags {
            flag.check();
        }
        Ok(())
    }

    fn get_flag_value (&self, name: &str) -> &Value {
        if let Ok(ref flag) = self.flags_by_long_ref(name) {
           if let Value::Error(ref e) = flag.value {
               self.quit(&format!("flag '{}': {}",name,e))
           }
           &flag.value
        } else {
            self.quit(&format!("unknown flag '{}'",name))
        }
    }

    fn bad_flag (&self, name: &str, tname: &str) -> ! {
        self.quit(&format!("flag '{}' is not a {}",name,tname))
    }

    /// get flag as a string, quitting otherwise.
    pub fn get_string(&self, name: &str) -> String {
        let s = self.get_flag_value(name).as_string();
        if s.is_some() {s.unwrap()} else {self.bad_flag(name,"string")}
    }

    /// get flag as an integer, quitting otherwise.
    pub fn get_integer(&self, name: &str) -> i32 {
        let s = self.get_flag_value(name).as_int();
        if s.is_some() {s.unwrap()} else {self.bad_flag(name,"integer")}
    }

    /// get flag as a float, quitting otherwise.
    pub fn get_float(&self, name: &str) -> f32 {
        let s = self.get_flag_value(name).as_float();
        if s.is_some() {s.unwrap()} else {self.bad_flag(name,"float")}
    }

    /// get flag as a bool, quitting otherwise.
    pub fn get_bool(&self, name: &str) -> bool {
        let s = self.get_flag_value(name).as_bool();
        if s.is_some() {s.unwrap()} else {self.bad_flag(name,"bool")}
    }

    fn get_boxed_array(&self, name: &str, kind: &str) -> &Vec<Box<Value>> {
        match self.get_flag_value(name).as_array() {
            None => self.bad_flag(name,"array"),
            Some(arr) => {
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
    where T: Sized, F: Fn(&Box<Value>)->Option<T> {
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
  -o, --output (default stdout)
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
