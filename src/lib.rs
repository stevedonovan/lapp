//! `lapp` provides a straightforward way to parse command-line
//! arguments, using the _usage text_ as a pattern.
//!
//! # Example
//! ```
//! let args = lapp::Args::new("
//!    A test program
//!    -v,--verbose  verbose output
//!    -s, --save (default 'out.txt')	
//! ");
//! assert_eq!(args.get_bool("verbose"),false);
//! assert_eq!(args.get_string("save"),"out.txt".to_string());
//! ```

	

extern crate scanlex;
use scanlex::Scanner;
use scanlex::Token;
use std::process;
use std::error::Error;

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
    fn from_name(s: &str) -> Result<Type,String> {
        match s {
        "string" => Ok(Type::Str),
        "integer" => Ok(Type::Int),
        "float" => Ok(Type::Float),
        "bool" => Ok(Type::Bool),        
        _ => Err(format!("not a known type {}",s))
        }
    }
    
    fn array_type(&self) -> Option<&Type> {
        // &**bt means (from R to L) deref bt, unbox type, return reference
        match *self {Type::Arr(ref bt) => Some(&**bt), _ => None}
    }
    
    fn box_type(self) -> Type {
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
            let mut res = Vec::new();
            for part in s.split_whitespace() {
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
        Value::Arr(ref v) => (*v[0]).type_of()
        }
    }
    
    fn is_err(&self) -> bool {
        if let Value::Error(_) = *self {true} else {false}
    }

    fn from_value (tok: &Token) -> Result<Value,String> {
        match *tok {
        Token::Str(ref s) => Ok(Value::Str(s.clone())),
        Token::Num(x) => if x.floor()==x {
                Ok(Value::Int(x as i32))
            } else {
                Ok(Value::Float(x as f32))
            },
        Token::Iden(ref s) => Ok(Value::Str(s.clone())),
        Token::Char(ch) => Ok(Value::Str(ch.to_string())),
        _ => Err(format!("bad default value {:?}",tok))
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

macro_rules! dbg {
    ($x:expr) => {
        println!("{} = {:?}",concat!(file!(),':',line!(),' ',stringify!($x)),$x);
    }
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
				} else {
					self.value = Value::Error(format!("required flag {}",self.long));
				}
            } else {
                self.value = self.defval.clone();
            }
        }
    }
    
    fn declaration(&self) {
        // long name may need massaging to become a Rust variable name
        // The result must be snake_case to keep compiler happy!
        let mut name = self.long.replace('-',"_").to_lowercase();
        let firstc = name.chars().nth(0).unwrap();
        if firstc.is_digit(10) || firstc == '_' {
            name = format!("c_{}",name);
        }
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
        println!("    let {} = args.get_{}(\"{}\");",
            name,tname,self.long);
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
    
    /// bail out of program with non-zero return code
    pub fn quit(&self, msg: &str) -> ! {
        println!("error: {}",msg);
        println!("usage: {}",self.text);
        process::exit(1);
    }
    
    /// print out suggested variable declarations for accessing the flags...
    pub fn declarations(&mut self) {
        self.parse_spec();
        for f in &self.flags {
            f.declaration();
        }
    }
    
    pub fn dump(self) {
        for f in &self.flags {
            println!("flag '{}' value {:?}",f.long,f.value);
        }
    }
    
    pub fn parse(&mut self) {
        self.parse_spec();
        if let Err(e) = self.parse_args() { self.quit(&e) }
    }
    
    pub fn parse_spec(&mut self) {
        for line in self.text.lines() {
            if let Err(e) = self.parse_spec_line(line) {
                self.quit(&e);
            }
        }
    }

    fn parse_spec_line(&mut self, line: &str) -> Result<(),String> {
        let mut scan = Scanner::new(line);
        if ! scan.skip_whitespace() { return Ok(()); } // empty!
        
        // either (flags) -short, --long, --long or -short
        // or (arguments) <name>
        // maybe followed by a type specifier (...) (otherwise just a bool flag)
        // Rest of line is just bumpf.
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
                    return Err(format!("{:?} isn't short: only letters or digits allowed",short));
                }
                if ch != ',' && ch != ' ' {
                    return Err(format!("{:?} isn't short: not followed by comma or space",short));
                }
                if ch == ',' { // expecting long flag!
                    if scan.peek() == ' ' {
                        scan.nextch();
                    }
                    ch = scan.nextch();
                    if ch != '-' {
                        return Err(format!("expected long flag after short flag {:?}",short));
                    }
                }
            }
            if ch == '-' { // long flag nane
                scan.nextch(); // skip -
                long = scan.grab_while(|c| c.is_alphanumeric() || c == '-' || c == '_');
                
            }
            if long == "" { 
                long = short.to_string();
            }
            // flag followed by '...' means that the flag may
            // occur multiple times, with the results stored in a vector
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
                flag_type = flag_type.box_type();
                default_val = Value::empty_array();
                if is_positional {
					is_multiple = true;
				}
            }
			if is_multiple {
				flag_value = Value::empty_array();
			}                        
        } else {
            default_val = Value::Bool(false);
        }
        
        if self.flags_by_long_ref(&long).is_ok() {
            return Err(format!("flag {} already defined",long));
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
    
    fn flags_by_long(&mut self, s: &str) -> Result<&mut Flag,String> {
        self.flags.iter_mut()
            .filter(|&ref f| f.long == s)
            .next().ok_or(format!("no long flag '{}'",s))       
    }
    
    fn flags_by_long_ref(&self, s: &str) -> Result<&Flag,String> {
        self.flags.iter()
            .filter(|&f| f.long == s)
            .next().ok_or(format!("no long flag '{}'",s))
    }    
    
    fn flags_by_short(&mut self, ch: char) -> Result<&mut Flag,String> {
        self.flags.iter_mut()
            .filter(|&ref f| f.short == ch)
            .next().ok_or(format!("no short flag '{}'",ch))
    }
    
    fn flags_by_pos(&mut self, pos: usize) -> Result<&mut Flag,String> {
        self.flags.iter_mut()
            .filter(|&ref f| f.pos == pos)
            .next().ok_or(format!("no positional arg '{}'",pos))
    }
    
    fn parse_args(&mut self) -> Result<(),String> {
        let mut iter = std::env::args().skip(1);
        
        fn nextarg(name: &str, ms: Option<String>) -> Result<String,String> {
            if  ms.is_none() {return Err(format!("no value for flag '{}'",name));}
            let res = ms.unwrap();
            if res.starts_with('-') {return Err(format!("flag '{}' is expecting value",name));}
            Ok(res)
        };
        
        while let Some(s) = iter.next() {
            let mut k = 0;
            let mut scan = Scanner::new(&s);
            let mut ch = scan.nextch();
            if ch == '-' {
                if scan.peek() == '-' { // long flag
                    scan.nextch();
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
                    }
                } else {
                    loop {
                        // similar, except there can be multiple short flags
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
                k += 1;
                let mut flag = try!(self.flags_by_pos(k));
                flag.set_value_from_string(&s);
            }
        }
        self.check_flags();
        Ok(())
    }
    
  
    fn check_flags(&mut self) {
        for flag in &mut self.flags {
            flag.check();            
        }        
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
    
    pub fn get_string(&self, name: &str) -> String {
        let s = self.get_flag_value(name).as_string();
        if s.is_some() {s.unwrap()} else {self.bad_flag(name,"string")}
    }
    
    pub fn get_int(&self, name: &str) -> i32 {
        let s = self.get_flag_value(name).as_int();
        if s.is_some() {s.unwrap()} else {self.bad_flag(name,"integer")}
    }
    
    pub fn get_float(&self, name: &str) -> f32 {
        let s = self.get_flag_value(name).as_float();
        if s.is_some() {s.unwrap()} else {self.bad_flag(name,"float")}
    }
    
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
    
    pub fn get_strings(self, name: &str) -> Vec<String> {
		self.get_array(name,"string",|b| b.as_string())
	}
    
    pub fn get_ints(self, name: &str) -> Vec<i32> {
		self.get_array(name,"integer",|b| b.as_int())
	}
	
    pub fn get_floats(self, name: &str) -> Vec<f32> {
		self.get_array(name,"float",|b| b.as_float())
	}
	
    
}

