use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use crate::diagnostics::*;
use crate::lexer::Loc;
use crate::parser::parse_all;
use crate::runtime::Env;
use crate::runtime::execute_asts;
use crate::types::intern;
use crate::types::lookup;
use crate::value::*;

type Result<T> = std::result::Result<T, SelError>;

#[inline]
pub fn sum_slice(loc: Loc, args: &[Value]) -> Result<Value> {
    let mut int_sum = 0i64;
    let mut float_sum = 0.0f64;
    let mut is_float = false;

    for arg in args {
        match arg {
            Value::Integer(i) => {
                if is_float {
                    float_sum += *i as f64;
                } else {
                    int_sum += *i;
                }
            }
            Value::Float(f) => {
                if !is_float {
                    is_float = true;
                    float_sum = int_sum as f64 + *f;
                } else {
                    float_sum += *f;
                }
            }
            v => {
                return Err(SelError::TypeError(
                    loc,
                    format!(
                        "Invalid argument to +: expected number but got {}",
                        value_type_name(v)
                    ),
                ));
            }
        }
    }
    if is_float {
        Ok(Value::Float(float_sum))
    } else {
        Ok(Value::Integer(int_sum))
    }
}

#[inline]
pub fn sum(loc: Loc, args: Vec<Value>) -> Result<Value> {
    sum_slice(loc, &args)
}

#[inline]
pub fn sub_slice(loc: Loc, args: &[Value]) -> Result<Value> {
    if args.is_empty() {
        return Err(SelError::Runtime(
            loc,
            "Expected at least 1 argument for -".into(),
        ));
    }
    let mut is_float = false;
    let mut int_val = 0i64;
    let mut float_val = 0.0f64;

    match &args[0] {
        Value::Integer(i) => int_val = *i,
        Value::Float(f) => {
            is_float = true;
            float_val = *f;
        }
        v => {
            return Err(SelError::TypeError(
                loc,
                format!(
                    "Invalid argument to -: expected number but got {}",
                    value_type_name(v)
                ),
            ));
        }
    }

    if args.len() == 1 {
        return if is_float {
            Ok(Value::Float(-float_val))
        } else {
            Ok(Value::Integer(-int_val))
        };
    }

    for arg in &args[1..] {
        match arg {
            Value::Integer(i) => {
                if is_float {
                    float_val -= *i as f64;
                } else {
                    int_val -= *i;
                }
            }
            Value::Float(f) => {
                if !is_float {
                    is_float = true;
                    float_val = int_val as f64 - *f;
                } else {
                    float_val -= *f;
                }
            }
            _ => {
                return Err(SelError::Runtime(
                    loc,
                    "Invalid argument to -: expected number".into(),
                ));
            }
        }
    }
    if is_float {
        Ok(Value::Float(float_val))
    } else {
        Ok(Value::Integer(int_val))
    }
}

#[inline]
pub fn sub(loc: Loc, args: Vec<Value>) -> Result<Value> {
    sub_slice(loc, &args)
}

#[inline]
pub fn mul_slice(loc: Loc, args: &[Value]) -> Result<Value> {
    let mut int_val = 1i64;
    let mut float_val = 1.0f64;
    let mut is_float = false;

    for arg in args {
        match arg {
            Value::Integer(i) => {
                if is_float {
                    float_val *= *i as f64;
                } else {
                    int_val *= *i;
                }
            }
            Value::Float(f) => {
                if !is_float {
                    is_float = true;
                    float_val = int_val as f64 * *f;
                } else {
                    float_val *= *f;
                }
            }
            _ => {
                return Err(SelError::Runtime(
                    loc,
                    "Invalid argument to *: expected number".into(),
                ));
            }
        }
    }
    if is_float {
        Ok(Value::Float(float_val))
    } else {
        Ok(Value::Integer(int_val))
    }
}

#[inline]
pub fn mul(loc: Loc, args: Vec<Value>) -> Result<Value> {
    mul_slice(loc, &args)
}

#[inline]
pub fn div_slice(loc: Loc, args: &[Value]) -> Result<Value> {
    if args.is_empty() {
        return Err(SelError::Runtime(
            loc,
            "Expected at least 1 argument for /".into(),
        ));
    }
    if args.len() == 1 {
        match args[0] {
            Value::Integer(i) => return Ok(Value::Float(1.0 / i as f64)),
            Value::Float(f) => return Ok(Value::Float(1.0 / f)),
            _ => {
                return Err(SelError::Runtime(
                    loc,
                    "Invalid argument to /: expected number".into(),
                ));
            }
        }
    }

    let mut float_val = match args[0] {
        Value::Integer(i) => i as f64,
        Value::Float(f) => f,
        _ => {
            return Err(SelError::Runtime(
                loc,
                "Invalid argument to /: expected number".into(),
            ));
        }
    };

    for arg in &args[1..] {
        match arg {
            Value::Integer(i) => float_val /= *i as f64,
            Value::Float(f) => float_val /= *f,
            _ => {
                return Err(SelError::Runtime(
                    loc,
                    "Invalid argument to /: expected number".into(),
                ));
            }
        }
    }
    Ok(Value::Float(float_val))
}

#[inline]
pub fn div(loc: Loc, args: Vec<Value>) -> Result<Value> {
    div_slice(loc, &args)
}

#[inline]
pub fn modulo_slice(loc: Loc, args: &[Value]) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::Runtime(
            loc,
            "Expected 2 arguments for mod".into(),
        ));
    }
    let a = match args[0] {
        Value::Integer(i) => i,
        _ => {
            return Err(SelError::Runtime(loc, "modulo requires integer".into()));
        }
    };
    let b = match args[1] {
        Value::Integer(i) => i,
        _ => {
            return Err(SelError::Runtime(loc, "modulo requires integer".into()));
        }
    };
    if b == 0 {
        return Err(SelError::Runtime(loc, "division by zero in modulo".into()));
    }
    Ok(Value::Integer(a.checked_rem(b).unwrap_or(0)))
}

#[inline]
pub fn modulo(loc: Loc, args: Vec<Value>) -> Result<Value> {
    modulo_slice(loc, &args)
}

#[inline]
fn compare_nums_slice(loc: Loc, args: &[Value], op: fn(f64, f64) -> bool) -> Result<Value> {
    if args.is_empty() {
        return Ok(Value::Boolean(true));
    }
    let mut prev = match args[0] {
        Value::Integer(i) => i as f64,
        Value::Float(f) => f,
        _ => {
            return Err(SelError::Runtime(loc, "comparison requires numbers".into()));
        }
    };
    for arg in &args[1..] {
        let curr = match arg {
            Value::Integer(i) => *i as f64,
            Value::Float(f) => *f,
            _ => {
                return Err(SelError::Runtime(loc, "comparison requires numbers".into()));
            }
        };
        if !op(prev, curr) {
            return Ok(Value::Boolean(false));
        }
        prev = curr;
    }
    Ok(Value::Boolean(true))
}

#[inline]
pub fn is_equal_slice(_loc: Loc, args: &[Value]) -> Result<Value> {
    if args.len() < 2 {
        return Ok(Value::Boolean(true));
    }
    let first = &args[0];
    for arg in &args[1..] {
        let eq = is_value_equal(first, arg);
        if !eq {
            return Ok(Value::Boolean(false));
        }
    }
    Ok(Value::Boolean(true))
}

#[inline]
pub fn is_equal(loc: Loc, args: Vec<Value>) -> Result<Value> {
    is_equal_slice(loc, &args)
}

#[inline]
fn is_value_equal(first: &Value, arg: &Value) -> bool {
    match (first, arg) {
        (Value::Nil, Value::Nil) => true,
        (Value::Nil, Value::List(l)) if l.is_empty() => true,
        (Value::List(l), Value::Nil) if l.is_empty() => true,
        (Value::Nil, Value::String(s)) if s.is_empty() => true,
        (Value::String(s), Value::Nil) if s.is_empty() => true,
        (Value::List(l), Value::String(s)) if l.is_empty() && s.is_empty() => true,
        (Value::String(s), Value::List(l)) if s.is_empty() && l.is_empty() => true,
        (Value::Boolean(a), Value::Boolean(b)) => a == b,
        (Value::Integer(a), Value::Integer(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Symbol(a), Value::Symbol(b)) => a == b,
        (Value::Pointer(a), Value::Pointer(b)) => a == b,
        (Value::String(a), Value::String(b)) => a.as_str() == b.as_str(),
        (Value::String(s), Value::List(l)) | (Value::List(l), Value::String(s)) => {
            s.char_count() == l.len()
                && s.as_str().chars().zip(l.iter()).all(|(c, v)| match v {
                    Value::Char(vc) => c == *vc,
                    _ => false,
                })
        }
        (Value::List(sa), Value::List(sb)) => {
            sa.len() == sb.len() && sa.iter().zip(sb.iter()).all(|(a, b)| is_value_equal(a, b))
        }
        (Value::Record(a), Value::Record(b)) => {
            a.fields().len() == b.fields().len()
                && a.fields()
                    .iter()
                    .all(|(ka, va)| b.fields().get(ka).is_some_and(|vb| is_value_equal(va, vb)))
        }
        (Value::Char(a), Value::Char(b)) => a == b,
        _ => false,
    }
}

#[inline]
pub fn num_noteq_slice(loc: Loc, args: &[Value]) -> Result<Value> {
    compare_nums_slice(loc, args, |a, b| a != b)
}
#[inline]
pub fn num_noteq(loc: Loc, args: Vec<Value>) -> Result<Value> {
    num_noteq_slice(loc, &args)
}
#[inline]
pub fn num_eq_slice(loc: Loc, args: &[Value]) -> Result<Value> {
    compare_nums_slice(loc, args, |a, b| a == b)
}
#[inline]
pub fn num_eq(loc: Loc, args: Vec<Value>) -> Result<Value> {
    num_eq_slice(loc, &args)
}
#[inline]
pub fn num_lt_slice(loc: Loc, args: &[Value]) -> Result<Value> {
    compare_nums_slice(loc, args, |a, b| a < b)
}
#[inline]
pub fn num_lt(loc: Loc, args: Vec<Value>) -> Result<Value> {
    num_lt_slice(loc, &args)
}
#[inline]
pub fn num_gt_slice(loc: Loc, args: &[Value]) -> Result<Value> {
    compare_nums_slice(loc, args, |a, b| a > b)
}
#[inline]
pub fn num_gt(loc: Loc, args: Vec<Value>) -> Result<Value> {
    num_gt_slice(loc, &args)
}
#[inline]
pub fn num_lte_slice(loc: Loc, args: &[Value]) -> Result<Value> {
    compare_nums_slice(loc, args, |a, b| a <= b)
}
#[inline]
pub fn num_lte(loc: Loc, args: Vec<Value>) -> Result<Value> {
    num_lte_slice(loc, &args)
}
#[inline]
pub fn num_gte_slice(loc: Loc, args: &[Value]) -> Result<Value> {
    compare_nums_slice(loc, args, |a, b| a >= b)
}
#[inline]
pub fn num_gte(loc: Loc, args: Vec<Value>) -> Result<Value> {
    num_gte_slice(loc, &args)
}

#[inline]
pub fn error(loc: Loc, args: Vec<Value>) -> Result<Value> {
    let msg = args
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    Err(SelError::Runtime(loc, msg))
}

#[inline]
pub fn value_type_name(v: &Value) -> &str {
    match v {
        Value::Nil => "nil",
        Value::Integer(_) => "int",
        Value::Float(_) => "float",
        Value::Boolean(_) => "bool",
        Value::Symbol(_) => "symbol",
        Value::String(_) => "string",
        Value::List(l) => {
            if !l.is_empty() && l.iter().all(|v| matches!(v, Value::Char(_))) {
                "string"
            } else {
                "list"
            }
        }
        Value::NativeClosure(_) | Value::Closure(_) | Value::NativeFunction(_) => "function",
        Value::Macro { .. } => "macro",
        Value::Pointer(_) => "pointer",
        #[cfg(feature = "ffi")]
        Value::Library(_) => "library",
        Value::Record(_) => "record",
        Value::Coroutine(_) => "coroutine",
        Value::Char(_) => "char",
    }
}

#[inline]
pub fn not(_loc: Loc, args: Vec<Value>) -> Result<Value> {
    match args[0] {
        Value::Boolean(false) => Ok(Value::Boolean(true)),
        _ => Ok(Value::Boolean(false)),
    }
}

thread_local! {
    pub static OUTPUT_SINK: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

pub fn enable_output_capture() {
    OUTPUT_SINK.with(|sink| {
        *sink.borrow_mut() = Some(Vec::new());
    });
}

pub fn drain_captured_output() -> Vec<String> {
    OUTPUT_SINK.with(|sink| {
        sink.borrow_mut()
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    })
}

pub fn disable_output_capture() -> Vec<String> {
    OUTPUT_SINK.with(|sink| sink.borrow_mut().take().unwrap_or_default())
}

pub fn emit_stdout(s: &str) {
    let captured = OUTPUT_SINK.with(|sink| {
        if let Some(buf) = sink.borrow_mut().as_mut() {
            buf.push(s.to_string());
            true
        } else {
            false
        }
    });
    if !captured {
        print!("{}", s);
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }
}

pub fn display_newline(loc: Loc, args: Vec<Value>) -> Result<Value> {
    display(loc, args)?;
    newline(loc, Vec::new())
}

pub fn display(_loc: Loc, args: Vec<Value>) -> Result<Value> {
    let mut out = String::new();
    for (i, arg) in args.into_iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&arg.to_string());
    }
    emit_stdout(&out);
    Ok(Value::Nil)
}

#[cfg(feature = "ffi")]
pub fn ffi_dlopen(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for ffi-dlopen".into(),
        ));
    }
    if let Some(s) = args[0].to_string_lossy() {
        unsafe {
            match libloading::Library::new(&s) {
                Ok(lib) => Ok(Value::Library(Rc::new(lib))),
                Err(e) => Err(SelError::Runtime(loc, format!("dlopen failed: {}", e))),
            }
        }
    } else {
        Err(SelError::Runtime(
            loc,
            "ffi-dlopen requires a string".into(),
        ))
    }
}

#[cfg(feature = "ffi")]
pub fn ffi_dlsym(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 2 arguments for ffi-dlsym".into(),
        ));
    }
    let lib = match &args[0] {
        Value::Library(l) => l,
        _ => {
            return Err(SelError::Runtime(
                loc,
                "ffi-dlsym requires a library".into(),
            ));
        }
    };
    let sym_name = match args[1].to_string_lossy() {
        Some(s) => s,
        None => {
            return Err(SelError::Runtime(
                loc,
                "ffi-dlsym requires a string symbol name".into(),
            ));
        }
    };

    let mut sym_bytes = sym_name.into_bytes();
    sym_bytes.push(0);

    unsafe {
        match lib.get::<*const ()>(&*sym_bytes) {
            Ok(sym) => {
                let ptr = *sym as usize;
                Ok(Value::Pointer(ptr))
            }
            Err(e) => Err(SelError::Runtime(loc, format!("dlsym failed: {}", e))),
        }
    }
}

#[cfg(feature = "ffi")]
#[derive(Debug, Clone)]
enum FfiType {
    Void,
    I8,
    U8,
    I16,
    U16,
    I32,
    I64,
    U32,
    U64,
    F32,
    F64,
    Bool,
    Char,
    CStr,
    Pointer,
    Struct(Vec<FfiType>),
}

#[cfg(feature = "ffi")]
impl FfiType {
    fn size_and_alignment(&self) -> (usize, usize) {
        match self {
            FfiType::Void => (0, 1),
            FfiType::I8 | FfiType::U8 | FfiType::Bool | FfiType::Char => (1, 1),
            FfiType::I16 | FfiType::U16 => (2, 2),
            FfiType::I32 | FfiType::U32 | FfiType::F32 => (4, 4),
            FfiType::I64 | FfiType::U64 | FfiType::F64 | FfiType::Pointer | FfiType::CStr => (8, 8),
            FfiType::Struct(fields) => {
                let mut current_offset = 0;
                let mut max_align = 1;
                for field in fields {
                    let (f_size, f_align) = field.size_and_alignment();
                    if f_align > max_align {
                        max_align = f_align;
                    }
                    current_offset = (current_offset + f_align - 1) & !(f_align - 1);
                    current_offset += f_size;
                }
                let total_size = (current_offset + max_align - 1) & !(max_align - 1);
                (total_size, max_align)
            }
        }
    }

    fn to_libffi_type(&self) -> libffi::middle::Type {
        match self {
            FfiType::Void => libffi::middle::Type::void(),
            FfiType::I8 => libffi::middle::Type::i8(),
            FfiType::U8 => libffi::middle::Type::u8(),
            FfiType::I16 => libffi::middle::Type::i16(),
            FfiType::U16 => libffi::middle::Type::u16(),
            FfiType::I32 => libffi::middle::Type::i32(),
            FfiType::I64 => libffi::middle::Type::i64(),
            FfiType::U32 => libffi::middle::Type::u32(),
            FfiType::U64 => libffi::middle::Type::u64(),
            FfiType::F32 => libffi::middle::Type::f32(),
            FfiType::F64 => libffi::middle::Type::f64(),
            FfiType::Bool => libffi::middle::Type::u8(),
            FfiType::Char => libffi::middle::Type::i8(),
            FfiType::Pointer | FfiType::CStr => libffi::middle::Type::pointer(),
            FfiType::Struct(fields) => {
                let ffi_fields: Vec<_> = fields.iter().map(|f| f.to_libffi_type()).collect();
                libffi::middle::Type::structure(ffi_fields)
            }
        }
    }
}

#[cfg(feature = "ffi")]
fn parse_ffi_type(loc: Loc, val: &Value) -> Result<FfiType> {
    match val {
        Value::Symbol(s) => {
            let sym = crate::lookup(*s);
            match sym.as_str() {
                "void" => Ok(FfiType::Void),
                "i8" | "ichar" => Ok(FfiType::I8),
                "char" => Ok(FfiType::Char),
                "u8" | "uchar" => Ok(FfiType::U8),
                "i16" => Ok(FfiType::I16),
                "u16" => Ok(FfiType::U16),
                "i32" => Ok(FfiType::I32),
                "i64" => Ok(FfiType::I64),
                "u32" => Ok(FfiType::U32),
                "u64" => Ok(FfiType::U64),
                "f32" => Ok(FfiType::F32),
                "f64" => Ok(FfiType::F64),
                "bool" => Ok(FfiType::Bool),
                "*void" => Ok(FfiType::Pointer),
                "string" => Ok(FfiType::CStr),
                _ => Err(SelError::Runtime(
                    loc,
                    format!("Unsupported FFI primitive type: {sym}"),
                )),
            }
        }
        Value::List(l) => {
            if l.len() != 2 {
                return Err(SelError::Runtime(
                    loc,
                    "Invalid FFI type list: expected (struct (type1 type2 ...))".into(),
                ));
            }
            let first_sym = match &l[0] {
                Value::Symbol(s) => crate::lookup(*s),
                _ => {
                    return Err(SelError::Runtime(
                        loc,
                        "Expected symbol as first element of FFI type list".into(),
                    ));
                }
            };
            if first_sym != "struct" {
                return Err(SelError::Runtime(
                    loc,
                    format!("Expected 'struct' keyword but got {first_sym}"),
                ));
            }
            let fields_slice = match &l[1] {
                Value::List(fl) => fl,
                Value::Nil => return Ok(FfiType::Struct(Vec::new())),
                _ => {
                    return Err(SelError::Runtime(
                        loc,
                        "Expected list of fields in struct type definition".into(),
                    ));
                }
            };
            let mut fields = Vec::with_capacity(fields_slice.len());
            for field in fields_slice.iter() {
                fields.push(parse_ffi_type(loc, field)?);
            }
            Ok(FfiType::Struct(fields))
        }
        _ => Err(SelError::Runtime(loc, format!("Invalid FFI type: {val}"))),
    }
}

#[cfg(feature = "ffi")]
fn serialize_value(
    loc: Loc,
    val: &Value,
    ty: &FfiType,
    buf: &mut Vec<u8>,
    c_strings: &mut Vec<std::ffi::CString>,
) -> Result<()> {
    macro_rules! write_num {
        ($t:ty, $name:literal) => {{
            let n: $t = match val {
                Value::Integer(i) => *i as $t,
                Value::Float(f) => *f as $t,
                _ => {
                    return Err(SelError::Runtime(
                        loc,
                        format!(
                            "Expected integer/float for {} but got {}",
                            $name,
                            value_type_name(val)
                        ),
                    ));
                }
            };
            buf.extend_from_slice(&n.to_ne_bytes());
            Ok(())
        }};
    }
    match ty {
        FfiType::Void => Ok(()),
        FfiType::I16 => write_num!(i16, "i16"),
        FfiType::U16 => write_num!(u16, "u16"),
        FfiType::I32 => write_num!(i32, "i32"),
        FfiType::I64 => write_num!(i64, "i64"),
        FfiType::U32 => write_num!(u32, "u32"),
        FfiType::U64 => write_num!(u64, "u64"),
        FfiType::F32 => write_num!(f32, "f32"),
        FfiType::F64 => write_num!(f64, "f64"),
        FfiType::I8 => {
            let n = match val {
                Value::Boolean(b) => i8::from(*b),
                Value::Integer(i) => *i as i8,
                Value::Char(c) => *c as i8,
                _ => {
                    return Err(SelError::Runtime(
                        loc,
                        format!(
                            "Expected boolean, integer, or char for i8 but got {}",
                            value_type_name(val)
                        ),
                    ));
                }
            };
            buf.extend_from_slice(&n.to_ne_bytes());
            Ok(())
        }
        FfiType::Bool | FfiType::U8 => {
            let n = match val {
                Value::Boolean(b) => u8::from(*b),
                Value::Integer(i) => *i as u8,
                Value::Char(c) => *c as u8,
                _ => {
                    return Err(SelError::Runtime(
                        loc,
                        format!(
                            "Expected boolean, integer, or char for u8/bool but got {}",
                            value_type_name(val)
                        ),
                    ));
                }
            };
            buf.push(n);
            Ok(())
        }
        FfiType::Char => {
            let n = match val {
                Value::Char(c) => *c as u8,
                Value::Integer(i) => *i as u8,
                _ => {
                    return Err(SelError::Runtime(
                        loc,
                        format!(
                            "Expected char or integer for char but got {}",
                            value_type_name(val)
                        ),
                    ));
                }
            };
            buf.push(n);
            Ok(())
        }
        FfiType::Pointer | FfiType::CStr => {
            let ptr = match val {
                Value::Pointer(p) => *p,
                Value::Nil => 0,
                v if let Some(s_str) = v.to_string_lossy() => {
                    let cstr = std::ffi::CString::new(s_str).unwrap();
                    let ptr = cstr.as_ptr() as usize;
                    c_strings.push(cstr);
                    ptr
                }
                _ => {
                    return Err(SelError::Runtime(
                        loc,
                        format!(
                            "Expected string, pointer, or nil but got {}",
                            value_type_name(val)
                        ),
                    ));
                }
            };
            buf.extend_from_slice(&ptr.to_ne_bytes());
            Ok(())
        }
        FfiType::Struct(fields) => {
            let record_vals;
            let list = match val {
                Value::List(l) => l,
                Value::Record(r) => {
                    record_vals = r
                        .fields()
                        .iter()
                        .map(|(_, v)| v.clone())
                        .collect::<imbl::Vector<_>>();
                    &record_vals
                }
                _ => {
                    return Err(SelError::Runtime(
                        loc,
                        format!(
                            "Expected list or record for struct value but got {}",
                            value_type_name(val)
                        ),
                    ));
                }
            };
            if list.len() != fields.len() {
                return Err(SelError::Runtime(
                    loc,
                    format!(
                        "Struct value field count mismatch: expected {} but got {}",
                        fields.len(),
                        list.len()
                    ),
                ));
            }
            let mut current_offset = 0;
            for (f_val, f_type) in list.iter().zip(fields) {
                let (f_size, f_align) = f_type.size_and_alignment();
                let target_offset = (current_offset + f_align - 1) & !(f_align - 1);
                buf.resize(buf.len() + (target_offset - current_offset), 0);
                serialize_value(loc, f_val, f_type, buf, c_strings)?;
                current_offset = target_offset + f_size;
            }
            let (_, s_align) = ty.size_and_alignment();
            let total_size = (current_offset + s_align - 1) & !(s_align - 1);
            buf.resize(buf.len() + (total_size - current_offset), 0);
            Ok(())
        }
    }
}

#[cfg(feature = "ffi")]
unsafe fn deserialize_value(ty: &FfiType, ptr: *const u8) -> Value {
    unsafe {
        match ty {
            FfiType::Void => Value::Nil,
            FfiType::I8 => Value::Integer(std::ptr::read(ptr as *const i8) as i64),
            FfiType::U8 => Value::Integer(std::ptr::read(ptr) as i64),
            FfiType::I16 => Value::Integer(std::ptr::read_unaligned(ptr as *const i16) as i64),
            FfiType::U16 => Value::Integer(std::ptr::read_unaligned(ptr as *const u16) as i64),
            FfiType::I32 => Value::Integer(std::ptr::read_unaligned(ptr as *const i32) as i64),
            FfiType::U32 => Value::Integer(std::ptr::read_unaligned(ptr as *const u32) as i64),
            FfiType::I64 => Value::Integer(std::ptr::read_unaligned(ptr as *const i64)),
            FfiType::U64 => Value::Integer(std::ptr::read_unaligned(ptr as *const u64) as i64),
            FfiType::F32 => Value::Float(std::ptr::read_unaligned(ptr as *const f32) as f64),
            FfiType::F64 => Value::Float(std::ptr::read_unaligned(ptr as *const f64)),
            FfiType::Bool => Value::Boolean(std::ptr::read(ptr) != 0),
            FfiType::Char => Value::Char(std::ptr::read(ptr) as char),
            FfiType::Pointer | FfiType::CStr => {
                let val = std::ptr::read_unaligned(ptr as *const usize);
                if val == 0 {
                    Value::Nil
                } else {
                    Value::Pointer(val)
                }
            }
            FfiType::Struct(fields) => {
                let mut list = Vec::with_capacity(fields.len());
                let mut current_offset = 0;
                for f_type in fields {
                    let (f_size, f_align) = f_type.size_and_alignment();
                    let target_offset = (current_offset + f_align - 1) & !(f_align - 1);
                    list.push(deserialize_value(f_type, ptr.add(target_offset)));
                    current_offset = target_offset + f_size;
                }
                Value::make_list(list)
            }
        }
    }
}

#[cfg(feature = "ffi")]
pub fn ffi_call(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 4 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 4 arguments for ffi-call".into(),
        ));
    }
    let ptr = match args[0] {
        Value::Pointer(p) => p,
        _ => {
            return Err(SelError::Runtime(loc, "ffi-call requires a pointer".into()));
        }
    };

    let ret_type = parse_ffi_type(loc, &args[1])?;

    let arg_types_slice = match &args[2] {
        Value::List(l) => l,
        Value::Nil => &imbl::Vector::new(),
        _ => return Err(SelError::Runtime(loc, "arg_types must be a list".into())),
    };
    let mut arg_types = Vec::with_capacity(arg_types_slice.len());
    for t in arg_types_slice.iter() {
        arg_types.push(parse_ffi_type(loc, t)?);
    }

    let arg_vals_slice = match &args[3] {
        Value::List(l) => l,
        Value::Nil => &imbl::Vector::new(),
        _ => return Err(SelError::Runtime(loc, "arg_vals must be a list".into())),
    };
    if arg_vals_slice.len() != arg_types.len() {
        return Err(SelError::Runtime(
            loc,
            format!(
                "Argument count mismatch: expected {} but got {}",
                arg_types.len(),
                arg_vals_slice.len()
            ),
        ));
    }

    let ffi_ret_type = ret_type.to_libffi_type();
    let ffi_arg_types: Vec<_> = arg_types.iter().map(|t| t.to_libffi_type()).collect();
    let cif = libffi::middle::Cif::new(ffi_arg_types, ffi_ret_type);

    let mut arg_buffers = Vec::with_capacity(arg_vals_slice.len());
    let mut c_strings = Vec::new();
    for (arg_val, arg_type) in arg_vals_slice.iter().zip(&arg_types) {
        let mut buf = Vec::new();
        serialize_value(loc, arg_val, arg_type, &mut buf, &mut c_strings)?;
        arg_buffers.push(buf);
    }

    let mut call_args = Vec::with_capacity(arg_vals_slice.len());
    for buf in &arg_buffers {
        if buf.is_empty() {
            call_args.push(libffi::middle::arg(&0u8));
        } else {
            call_args.push(libffi::middle::arg(&buf[0]));
        }
    }

    let code_ptr = libffi::middle::CodePtr::from_ptr(ptr as *mut _);

    unsafe {
        match ret_type {
            FfiType::Void => {
                cif.call::<()>(code_ptr, &call_args);
                Ok(Value::Nil)
            }
            FfiType::I8 => {
                let res: i8 = cif.call(code_ptr, &call_args);
                Ok(Value::Integer(res as i64))
            }
            FfiType::I16 => {
                let res: i16 = cif.call(code_ptr, &call_args);
                Ok(Value::Integer(res as i64))
            }
            FfiType::U16 => {
                let res: u16 = cif.call(code_ptr, &call_args);
                Ok(Value::Integer(res as i64))
            }
            FfiType::Bool => {
                let res: u8 = cif.call(code_ptr, &call_args);
                Ok(Value::Boolean(res != 0))
            }
            FfiType::I32 => {
                let res: i32 = cif.call(code_ptr, &call_args);
                Ok(Value::Integer(res as i64))
            }
            FfiType::I64 => {
                let res: i64 = cif.call(code_ptr, &call_args);
                Ok(Value::Integer(res))
            }
            FfiType::U32 => {
                let res: u32 = cif.call(code_ptr, &call_args);
                Ok(Value::Integer(res as i64))
            }
            FfiType::U64 => {
                let res: u64 = cif.call(code_ptr, &call_args);
                Ok(Value::Integer(res as i64))
            }
            FfiType::F32 => {
                let res: f32 = cif.call(code_ptr, &call_args);
                Ok(Value::Float(res as f64))
            }
            FfiType::F64 => {
                let res: f64 = cif.call(code_ptr, &call_args);
                Ok(Value::Float(res))
            }
            FfiType::CStr => {
                let res: *const std::ffi::c_char = cif.call(code_ptr, &call_args);
                if res.is_null() {
                    Ok(Value::Nil)
                } else {
                    let c_str = std::ffi::CStr::from_ptr(res);
                    Ok(Value::make_string(&c_str.to_string_lossy()))
                }
            }
            FfiType::Pointer => {
                let res: *const std::ffi::c_void = cif.call(code_ptr, &call_args);
                if res.is_null() {
                    Ok(Value::Nil)
                } else {
                    Ok(Value::Pointer(res as usize))
                }
            }
            FfiType::U8 => {
                let res: u8 = cif.call(code_ptr, &call_args);
                Ok(Value::Integer(res as i64))
            }
            FfiType::Char => {
                let res: u8 = cif.call(code_ptr, &call_args);
                Ok(Value::Char(res as char))
            }
            FfiType::Struct(ref _fields) => {
                let res_val: [u64; 32] = cif.call(code_ptr, &call_args);
                let ptr = res_val.as_ptr() as *const u8;
                Ok(deserialize_value(&ret_type, ptr))
            }
        }
    }
}

#[inline]
pub fn cons(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::SyntaxError(
            loc,
            "Expected exactly 2 arguments for cons".into(),
        ));
    }
    let tail = args.pop().unwrap();
    let head = args.pop().unwrap();
    match tail {
        Value::List(mut l) => {
            l.push_front(head);
            Ok(Value::List(l))
        }
        Value::String(s) => match head {
            Value::Char(c) => {
                let mut res = String::with_capacity(c.len_utf8() + s.len());
                res.push(c);
                res.push_str(s.as_str());
                Ok(Value::make_string(&res))
            }
            _ => {
                let mut items = imbl::Vector::new();
                items.push_back(head);
                for c in s.as_str().chars() {
                    items.push_back(Value::Char(c));
                }
                Ok(Value::List(Box::new(items)))
            }
        },
        Value::Nil => Ok(Value::make_list(vec![head])),
        _ => Ok(Value::make_list(vec![head, tail])),
    }
}

#[inline]
pub fn car(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for car".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::List(mut l) => Ok(l.pop_front().unwrap_or(Value::Nil)),
        Value::String(s) => {
            if let Some(c) = s.as_str().chars().next() {
                Ok(Value::Char(c))
            } else {
                Ok(Value::Nil)
            }
        }
        _ => Err(SelError::Runtime(loc, "car requires a list".into())),
    }
}

#[inline]
pub fn cdr(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for cdr".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::List(mut l) => {
            *l = l.skip(1);
            if l.is_empty() {
                Ok(Value::Nil)
            } else {
                Ok(Value::List(l))
            }
        }
        Value::String(mut s) => {
            *s = s.drop_chars(1);
            if s.is_empty() {
                Ok(Value::Nil)
            } else {
                Ok(Value::String(s))
            }
        }
        _ => Err(SelError::Runtime(loc, "cdr requires a list".into())),
    }
}

#[inline]
pub fn nth(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 2 arguments for nth".into(),
        ));
    }
    let index = args.pop().unwrap();
    match args.pop().unwrap() {
        Value::List(l) => match index {
            Value::Integer(idx) => Ok(if idx >= 0 && (idx as usize) < l.len() {
                l[idx as usize].clone()
            } else {
                Value::Nil
            }),
            _ => Err(SelError::Runtime(loc, "nth requires an integer".into())),
        },
        Value::String(s) => match index {
            Value::Integer(idx) => {
                if idx >= 0 {
                    if let Some(c) = s.as_str().chars().nth(idx as usize) {
                        Ok(Value::Char(c))
                    } else {
                        Ok(Value::Nil)
                    }
                } else {
                    Ok(Value::Nil)
                }
            }
            _ => Err(SelError::Runtime(loc, "nth requires an integer".into())),
        },
        _ => Err(SelError::Runtime(loc, "nth requires a list".into())),
    }
}

#[inline]
pub fn drop(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 2 arguments for drop".into(),
        ));
    }
    let list_val = args.pop().unwrap();
    let n_val = args.pop().unwrap();
    let n = match n_val {
        Value::Integer(i) => {
            if i < 0 {
                0
            } else {
                i as usize
            }
        }
        _ => {
            return Err(SelError::Runtime(
                loc,
                "drop requires an integer count".into(),
            ));
        }
    };
    match list_val {
        Value::List(mut l) => {
            if n >= l.len() {
                Ok(Value::Nil)
            } else {
                *l = l.skip(n);
                Ok(Value::List(l))
            }
        }
        Value::String(mut s) => {
            *s = s.drop_chars(n);
            if s.is_empty() {
                Ok(Value::Nil)
            } else {
                Ok(Value::String(s))
            }
        }
        Value::Nil => Ok(Value::Nil),
        _ => Err(SelError::Runtime(loc, "drop requires a list".into())),
    }
}

#[inline]
pub fn count(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for count".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::List(l) => Ok(Value::Integer(l.len() as _)),
        Value::String(s) => Ok(Value::Integer(s.char_count() as _)),
        Value::Nil => Ok(Value::Integer(0)),
        _ => Err(SelError::Runtime(loc, "count requires a list".into())),
    }
}

#[inline]
pub fn list(_loc: Loc, args: Vec<Value>) -> Result<Value> {
    Ok(Value::make_list(args))
}

#[inline]
pub fn reverse(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 1,
            actual: args.len(),
        });
    }
    match args.pop().unwrap() {
        Value::List(l) => {
            if l.is_empty() {
                Ok(Value::Nil)
            } else {
                let rev: imbl::Vector<Value> = l.iter().rev().cloned().collect();
                Ok(Value::List(Box::new(rev)))
            }
        }
        Value::String(s) => {
            let rev: String = s.as_str().chars().rev().collect();
            Ok(Value::make_string(&rev))
        }
        Value::Nil => Ok(Value::Nil),
        v => Err(SelError::TypeError(
            loc,
            format!(
                "reverse requires a list or string, got {}",
                value_type_name(&v)
            ),
        )),
    }
}

#[inline]
pub fn append(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.is_empty() {
        return Ok(Value::Nil);
    }
    let mut acc = imbl::Vector::new();
    for arg in args {
        match arg {
            Value::List(l) => acc.append(*l),
            Value::String(s) => {
                for c in s.as_str().chars() {
                    acc.push_back(Value::Char(c));
                }
            }
            Value::Nil => {}
            v => {
                return Err(SelError::TypeError(
                    loc,
                    format!("append requires lists, got {}", value_type_name(&v)),
                ));
            }
        }
    }
    if acc.is_empty() {
        Ok(Value::Nil)
    } else {
        Ok(Value::List(Box::new(acc)))
    }
}

#[inline]
pub fn last(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 1,
            actual: args.len(),
        });
    }
    match args.pop().unwrap() {
        Value::List(l) => Ok(l.back().cloned().unwrap_or(Value::Nil)),
        Value::String(s) => Ok(s.as_str().chars().last().map_or(Value::Nil, Value::Char)),
        Value::Nil => Ok(Value::Nil),
        v => Err(SelError::TypeError(
            loc,
            format!(
                "last requires a list or string, got {}",
                value_type_name(&v)
            ),
        )),
    }
}

#[inline]
pub fn range(loc: Loc, args: Vec<Value>) -> Result<Value> {
    let (end, start, step) = match args.as_slice() {
        [Value::Integer(end)] => (*end, 0i64, 1i64),
        [Value::Integer(end), Value::Integer(start)] => (*end, *start, 1i64),
        [
            Value::Integer(end),
            Value::Integer(start),
            Value::Integer(step),
        ] => {
            if *step <= 0 {
                return Err(SelError::Runtime(loc, "range step must be positive".into()));
            }
            (*end, *start, *step)
        }
        _ => {
            return Err(SelError::Runtime(
                loc,
                "Arity/type mismatch in range: expected (range end), (range end start), or (range end start step)".into(),
            ));
        }
    };
    if start >= end {
        return Ok(Value::Nil);
    }
    let mut vec = imbl::Vector::new();
    let mut curr = start;
    while curr < end {
        vec.push_back(Value::Integer(curr));
        curr += step;
    }
    Ok(Value::List(Box::new(vec)))
}

#[inline]
pub fn empty(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::SyntaxError(
            loc,
            "Expected exactly 1 arguments for empty?".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::List(l) => Ok(Value::Boolean(l.is_empty())),
        Value::String(s) => Ok(Value::Boolean(s.is_empty())),
        Value::Nil => Ok(Value::Boolean(true)),
        v => Err(SelError::Runtime(
            loc,
            format!("empty requires a list got {v}"),
        )),
    }
}

#[inline]
pub fn rget(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 2 arguments for rget".into(),
        ));
    }
    let index = args.pop().unwrap();
    match args.pop().unwrap() {
        Value::Record(r) => match index {
            Value::Symbol(sym) => Ok(if let Some(v) = r.fields().get(&sym).cloned() {
                v
            } else {
                Value::Nil
            }),
            _ => Err(SelError::Runtime(loc, "rget requires a symbol".into())),
        },
        _ => Err(SelError::Runtime(loc, "rget requires a record".into())),
    }
}

#[inline]
pub fn rset(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 3 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 3 arguments for rset".into(),
        ));
    }
    let value = args.pop().unwrap();
    let index = args.pop().unwrap();
    match args.pop().unwrap() {
        Value::Record(r) => match index {
            Value::Symbol(sym) => {
                let mut new_r = (*r).clone();
                new_r.fields_mut().insert(sym, value);
                Ok(Value::Record(Rc::new(new_r)))
            }
            _ => Err(SelError::Runtime(loc, "rset requires a symbol".into())),
        },
        _ => Err(SelError::Runtime(loc, "rset requires a record".into())),
    }
}

#[inline]
pub fn rdel(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 2 arguments for rdel".into(),
        ));
    }
    let index = args.pop().unwrap();
    match args.pop().unwrap() {
        Value::Record(r) => match index {
            Value::Symbol(sym) => {
                let mut new_r = (*r).clone();
                new_r.fields_mut().shift_remove(&sym);
                Ok(Value::Record(Rc::new(new_r)))
            }
            _ => Err(SelError::Runtime(loc, "rdel requires a symbol".into())),
        },
        _ => Err(SelError::Runtime(loc, "rdel requires a record".into())),
    }
}

#[inline]
pub fn rkeys(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 argument for rkeys".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::Record(r) => {
            let keys_vec: Vec<Value> = r.fields().keys().map(|&k| Value::Symbol(k)).collect();
            Ok(Value::make_list(keys_vec))
        }
        _ => Err(SelError::Runtime(loc, "rkeys requires a record".into())),
    }
}

#[inline]
pub fn rvals(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 argument for rvals".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::Record(r) => {
            let vals_vec: Vec<Value> = r.fields().values().cloned().collect();
            Ok(Value::make_list(vals_vec))
        }
        _ => Err(SelError::Runtime(loc, "rvals requires a record".into())),
    }
}

#[inline]
pub fn rcontains(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 2 arguments for rcontains?".into(),
        ));
    }
    let index = args.pop().unwrap();
    match args.pop().unwrap() {
        Value::Record(r) => match index {
            Value::Symbol(sym) => Ok(Value::Boolean(r.fields().contains_key(&sym))),
            _ => Err(SelError::Runtime(
                loc,
                "rcontains? requires a symbol".into(),
            )),
        },
        _ => Err(SelError::Runtime(
            loc,
            "rcontains? requires a record".into(),
        )),
    }
}

#[inline]
fn unary_pred(loc: Loc, args: Vec<Value>, name: &str, pred: fn(&Value) -> bool) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            format!("Expected exactly 1 arguments for {name}"),
        ));
    }
    Ok(Value::Boolean(pred(&args[0])))
}

#[inline]
pub fn is_nil(loc: Loc, args: Vec<Value>) -> Result<Value> {
    unary_pred(loc, args, "nil?", |v| matches!(v, Value::Nil))
}

#[inline]
pub fn is_list(loc: Loc, args: Vec<Value>) -> Result<Value> {
    unary_pred(loc, args, "list?", |v| {
        matches!(v, Value::List(..) | Value::String(..))
    })
}

#[inline]
pub fn is_number(loc: Loc, args: Vec<Value>) -> Result<Value> {
    unary_pred(loc, args, "number?", |v| {
        matches!(v, Value::Integer(_) | Value::Float(_))
    })
}

#[inline]
pub fn is_string(loc: Loc, args: Vec<Value>) -> Result<Value> {
    unary_pred(loc, args, "string?", |v| match v {
        Value::String(..) => true,
        Value::List(l) => !l.is_empty() && l.iter().all(|x| matches!(x, Value::Char(_))),
        _ => false,
    })
}

#[inline]
pub fn string_contains(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 2 arguments for string-contains?".into(),
        ));
    }
    if let (Some(s_str), Some(sub_str)) = (args[0].to_string_lossy(), args[1].to_string_lossy()) {
        Ok(Value::Boolean(s_str.contains(&sub_str)))
    } else {
        Ok(Value::Boolean(false))
    }
}

#[inline]
pub fn is_symbol(loc: Loc, args: Vec<Value>) -> Result<Value> {
    unary_pred(loc, args, "symbol?", |v| matches!(v, Value::Symbol(_)))
}

static GENSYM_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[inline]
pub fn gensym(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() > 1 {
        return Err(SelError::Runtime(
            loc,
            "gensym takes 0 or 1 arguments".into(),
        ));
    }
    let prefix = if args.is_empty() {
        "g".to_string()
    } else {
        let val = args.pop().unwrap();
        match val {
            Value::Symbol(id) => lookup(id),
            v => v.to_string_lossy().ok_or_else(|| {
                SelError::Runtime(
                    loc,
                    format!("gensym: expected string or symbol prefix, got {v}"),
                )
            })?,
        }
    };
    let count = GENSYM_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(Value::Symbol(intern(&format!("{}_{}", prefix, count))))
}

#[inline]
pub fn is_record(loc: Loc, args: Vec<Value>) -> Result<Value> {
    unary_pred(loc, args, "record?", |v| matches!(v, Value::Record(_)))
}

#[inline]
pub fn is_function(loc: Loc, args: Vec<Value>) -> Result<Value> {
    unary_pred(loc, args, "function?", |v| {
        matches!(
            v,
            Value::Closure(_) | Value::NativeFunction(_) | Value::NativeClosure(_)
        )
    })
}

#[inline]
pub fn is_char(loc: Loc, args: Vec<Value>) -> Result<Value> {
    unary_pred(loc, args, "char?", |v| matches!(v, Value::Char(_)))
}

#[inline]
pub fn char_to_integer(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 argument for char->integer".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::Char(c) => Ok(Value::Integer(c as i64)),
        v => Err(SelError::Runtime(
            loc,
            format!(
                "char->integer: expected char, found {}",
                value_type_name(&v)
            ),
        )),
    }
}

#[inline]
pub fn integer_to_char(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 argument for integer->char".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::Integer(i) => {
            if let Some(c) = std::char::from_u32(i as u32) {
                Ok(Value::Char(c))
            } else {
                Err(SelError::Runtime(
                    loc,
                    format!("integer->char: invalid unicode scalar value {}", i),
                ))
            }
        }
        v => Err(SelError::Runtime(
            loc,
            format!(
                "integer->char: expected integer, found {}",
                value_type_name(&v)
            ),
        )),
    }
}

#[inline]
pub fn type_of(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for type-of".into(),
        ));
    }
    let v = args.pop().unwrap();
    Ok(Value::Symbol(intern(value_type_name(&v))))
}

pub fn newline(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if !args.is_empty() {
        return Err(SelError::Runtime(
            loc,
            "Expected exactly 1 arguments for newline".into(),
        ));
    }
    emit_stdout("\n");
    Ok(Value::Nil)
}

pub fn file_system(loc: Loc, call_args: Vec<Value>) -> Result<Value> {
    if call_args.is_empty() {
        return Err(SelError::SyntaxError(
            loc,
            "Expected symbol for system".into(),
        ));
    }
    system(loc, call_args)
}

fn fs_write(loc: Loc, args: &[Value]) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::SyntaxError(
            loc,
            "Expected exactly 2 arguments for write".into(),
        ));
    }
    if let (Some(path), Some(content)) = (args[0].to_string_lossy(), args[1].to_string_lossy()) {
        match crate::os::write(std::path::Path::new(&path), &content) {
            Ok(_) => Ok(Value::Nil),
            Err(e) => Err(SelError::Runtime(loc, format!("write failed: {}", e))),
        }
    } else {
        Err(SelError::Runtime(
            loc,
            "write requires string arguments".into(),
        ))
    }
}

fn fs_read(loc: Loc, args: &[Value]) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::SyntaxError(
            loc,
            "Expected exactly 1 argument for read".into(),
        ));
    }
    if let Some(path) = args[0].to_string_lossy() {
        match crate::os::read_to_string(std::path::Path::new(&path)) {
            Ok(content) => Ok(Value::make_string(&content)),
            Err(e) => Err(SelError::Runtime(loc, format!("read failed: {}", e))),
        }
    } else {
        Err(SelError::Runtime(
            loc,
            "read requires a string argument".into(),
        ))
    }
}

fn fs_list(loc: Loc, args: &[Value]) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::SyntaxError(
            loc,
            "Expected exactly 1 argument for list".into(),
        ));
    }
    if let Some(path) = args[0].to_string_lossy() {
        match crate::os::read_dir(std::path::Path::new(&path)) {
            Ok(entries) => {
                let list = entries
                    .into_iter()
                    .map(|name| Value::make_string(&name))
                    .collect::<Vec<_>>();
                Ok(Value::make_list(list))
            }
            Err(e) => Err(SelError::Runtime(loc, format!("list failed: {}", e))),
        }
    } else {
        Err(SelError::Runtime(
            loc,
            "list requires a string argument".into(),
        ))
    }
}

fn fs_delete(loc: Loc, args: &[Value]) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::SyntaxError(
            loc,
            "Expected exactly 1 argument for delete".into(),
        ));
    }
    if let Some(path) = args[0].to_string_lossy() {
        let p = std::path::Path::new(&path);
        match crate::os::remove(p) {
            Ok(_) => Ok(Value::Nil),
            Err(e) => Err(SelError::Runtime(loc, format!("delete failed: {}", e))),
        }
    } else {
        Err(SelError::Runtime(
            loc,
            "delete requires a string argument".into(),
        ))
    }
}

pub fn system(loc: Loc, mut system_args: Vec<Value>) -> Result<Value> {
    if system_args.is_empty() {
        return Err(SelError::SyntaxError(
            loc,
            "Expected symbol for system".into(),
        ));
    }
    let mut args = system_args.split_off(1);
    if let Some(Value::Symbol(sym)) = system_args.pop() {
        match lookup(sym).as_str() {
            "args" => {
                if !args.is_empty() {
                    return Err(SelError::SyntaxError(
                        loc,
                        "Expected exactly 0 arguments for args".into(),
                    ));
                }
                let args_vec = crate::os::args()
                    .into_iter()
                    .skip(1)
                    .map(|s| Value::make_string(&s))
                    .collect::<Vec<_>>();
                return Ok(Value::make_list(args_vec));
            }
            "getenv" | "get-env" => {
                if args.len() != 1 {
                    return Err(SelError::SyntaxError(
                        loc,
                        "Expected exactly 1 argument for getenv".into(),
                    ));
                }
                return if let Some(key) = args[0].to_string_lossy() {
                    match crate::os::get_env(&key) {
                        Some(val) => Ok(Value::make_string(&val)),
                        None => Ok(Value::Nil),
                    }
                } else {
                    Err(SelError::Runtime(
                        loc,
                        "getenv requires a string argument".into(),
                    ))
                };
            }
            "setenv" | "set-env" | "set-env!" => {
                if args.len() != 2 {
                    return Err(SelError::SyntaxError(
                        loc,
                        "Expected exactly 2 arguments for setenv".into(),
                    ));
                }
                let key = args[0]
                    .to_string_lossy()
                    .ok_or_else(|| SelError::Runtime(loc, "setenv requires string key".into()))?;
                let val = args[1]
                    .to_string_lossy()
                    .ok_or_else(|| SelError::Runtime(loc, "setenv requires string value".into()))?;
                crate::os::set_env(&key, &val);
                return Ok(Value::Nil);
            }
            "exit" => {
                if args.len() > 1 {
                    return Err(SelError::SyntaxError(
                        loc,
                        "Expected exactly 0 or 1 arguments for exit".into(),
                    ));
                }
                let code = match args.pop() {
                    Some(Value::Integer(code)) => code as _,
                    _ => 0,
                };
                crate::os::exit(code)
            }
            "sleep" => {
                if args.len() != 1 {
                    return Err(SelError::SyntaxError(
                        loc,
                        "Expected exactly 1 argument for sleep".into(),
                    ));
                }
                return if let Value::Integer(d) = &args[0] {
                    if *d > 0 {
                        crate::os::sleep(std::time::Duration::from_secs(*d as u64));
                    }
                    Ok(Value::Nil)
                } else {
                    Err(SelError::Runtime(
                        loc,
                        "sleep requires an integer argument".into(),
                    ))
                };
            }
            "sleep-ms" => {
                if args.len() != 1 {
                    return Err(SelError::SyntaxError(
                        loc,
                        "Expected exactly 1 argument for sleep-ms".into(),
                    ));
                }
                return if let Value::Integer(d) = &args[0] {
                    if *d > 0 {
                        crate::os::sleep(std::time::Duration::from_millis(*d as u64));
                    }
                    Ok(Value::Nil)
                } else {
                    Err(SelError::Runtime(
                        loc,
                        "sleep-ms requires an integer argument".into(),
                    ))
                };
            }
            "time" | "time-now-ms" => {
                if !args.is_empty() {
                    return Err(SelError::SyntaxError(
                        loc,
                        "Expected exactly 0 arguments for time".into(),
                    ));
                }
                let ms = crate::os::time_now_ms().map_err(|e| SelError::Runtime(loc, e))?;
                return Ok(Value::Integer(ms));
            }
            "cwd" | "current-dir" => {
                if !args.is_empty() {
                    return Err(SelError::SyntaxError(
                        loc,
                        "Expected exactly 0 arguments for cwd".into(),
                    ));
                }
                let cwd = crate::os::current_dir()
                    .map_err(|e| SelError::Runtime(loc, format!("cwd failed: {e}")))?;
                return Ok(Value::make_string(&cwd.to_string_lossy()));
            }
            "exists?" | "file-exists?" => {
                if args.len() != 1 {
                    return Err(SelError::SyntaxError(
                        loc,
                        "Expected exactly 1 argument for file-exists?".into(),
                    ));
                }
                return if let Some(path) = args[0].to_string_lossy() {
                    Ok(Value::Boolean(crate::os::exists(std::path::Path::new(
                        &path,
                    ))))
                } else {
                    Err(SelError::Runtime(
                        loc,
                        "file-exists? requires a string argument".into(),
                    ))
                };
            }
            "write" => return fs_write(loc, &args),
            "read" => return fs_read(loc, &args),
            "list" => return fs_list(loc, &args),
            "delete" => return fs_delete(loc, &args),
            _ => {}
        }
    }
    Err(SelError::SyntaxError(
        loc,
        "Expected symbol for system".into(),
    ))
}

pub fn co_create(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "co-create requires exactly 1 argument".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::Closure(closure) => {
            let co = Coroutine {
                state: std::cell::Cell::new(CoroutineState::Suspended),
                frames: RefCell::new(Vec::new()),
                operand_stack: RefCell::new(Vec::new()),
                closure,
            };
            Ok(Value::Coroutine(Rc::new(co)))
        }
        val => Err(SelError::Runtime(
            loc,
            format!("co-create: expected closure but got {}", val),
        )),
    }
}

pub fn co_state(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "co-state requires exactly 1 argument".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::Coroutine(co) => {
            let state_str = match co.state.get() {
                CoroutineState::Suspended => "suspended",
                CoroutineState::Running => "running",
                CoroutineState::Dead => "dead",
            };
            Ok(Value::Symbol(intern(state_str)))
        }
        val => Err(SelError::Runtime(
            loc,
            format!("co-state: expected coroutine but got {}", val),
        )),
    }
}

pub fn co_dead_p(loc: Loc, mut args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::Runtime(
            loc,
            "co-dead? requires exactly 1 argument".into(),
        ));
    }
    match args.pop().unwrap() {
        Value::Coroutine(co) => Ok(Value::Boolean(co.state.get() == CoroutineState::Dead)),
        val => Err(SelError::Runtime(
            loc,
            format!("co-dead?: expected coroutine but got {}", val),
        )),
    }
}

// Math primitives
#[inline]
fn num_as_f64(loc: Loc, v: &Value, fn_name: &str) -> Result<f64> {
    match v {
        Value::Integer(i) => Ok(*i as f64),
        Value::Float(f) => Ok(*f),
        _ => Err(SelError::TypeError(
            loc,
            format!("{fn_name}: expected number, found {}", value_type_name(v)),
        )),
    }
}

#[inline]
fn unary_f64_op(loc: Loc, args: Vec<Value>, fn_name: &str, op: fn(f64) -> f64) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 1,
            actual: args.len(),
        });
    }
    Ok(Value::Float(op(num_as_f64(loc, &args[0], fn_name)?)))
}

#[inline]
fn unary_round_op(loc: Loc, args: Vec<Value>, fn_name: &str, op: fn(f64) -> f64) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 1,
            actual: args.len(),
        });
    }
    match args[0] {
        Value::Integer(i) => Ok(Value::Integer(i)),
        Value::Float(f) => Ok(Value::Integer(op(f) as i64)),
        ref v => Err(SelError::TypeError(
            loc,
            format!("{fn_name}: expected number, found {}", value_type_name(v)),
        )),
    }
}

#[inline]
fn math_extremum(
    loc: Loc,
    args: Vec<Value>,
    fn_name: &str,
    cmp: fn(f64, f64) -> bool,
) -> Result<Value> {
    if args.is_empty() {
        return Err(SelError::Runtime(
            loc,
            format!("{fn_name} requires at least 1 argument"),
        ));
    }
    let mut best_val = args[0].clone();
    let mut best_f = num_as_f64(loc, &best_val, fn_name)?;
    for arg in args.into_iter().skip(1) {
        let f = num_as_f64(loc, &arg, fn_name)?;
        if cmp(f, best_f) {
            best_f = f;
            best_val = arg;
        }
    }
    Ok(best_val)
}

#[inline]
pub fn math_abs(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 1,
            actual: args.len(),
        });
    }
    match args[0] {
        Value::Integer(i) => Ok(Value::Integer(i.abs())),
        Value::Float(f) => Ok(Value::Float(f.abs())),
        ref v => Err(SelError::TypeError(
            loc,
            format!("abs: expected number, found {}", value_type_name(v)),
        )),
    }
}

#[inline]
pub fn math_min(loc: Loc, args: Vec<Value>) -> Result<Value> {
    math_extremum(loc, args, "min", |a, b| a < b)
}

#[inline]
pub fn math_max(loc: Loc, args: Vec<Value>) -> Result<Value> {
    math_extremum(loc, args, "max", |a, b| a > b)
}

#[inline]
pub fn math_sqrt(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 1,
            actual: args.len(),
        });
    }
    let f = num_as_f64(loc, &args[0], "sqrt")?;
    if f < 0.0 {
        return Err(SelError::Runtime(
            loc,
            "sqrt: cannot compute square root of negative number".into(),
        ));
    }
    Ok(Value::Float(f.sqrt()))
}

#[inline]
pub fn math_pow(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 2,
            actual: args.len(),
        });
    }
    if let (Value::Integer(base), Value::Integer(exp)) = (&args[0], &args[1])
        && *exp >= 0
        && *exp <= (u32::MAX as i64)
        && let Some(res) = base.checked_pow(*exp as u32)
    {
        return Ok(Value::Integer(res));
    }
    let base = num_as_f64(loc, &args[0], "pow")?;
    let exp = num_as_f64(loc, &args[1], "pow")?;
    Ok(Value::Float(base.powf(exp)))
}

#[inline]
pub fn math_floor(loc: Loc, args: Vec<Value>) -> Result<Value> {
    unary_round_op(loc, args, "floor", f64::floor)
}

#[inline]
pub fn math_ceil(loc: Loc, args: Vec<Value>) -> Result<Value> {
    unary_round_op(loc, args, "ceil", f64::ceil)
}

#[inline]
pub fn math_round(loc: Loc, args: Vec<Value>) -> Result<Value> {
    unary_round_op(loc, args, "round", f64::round)
}

#[inline]
pub fn math_sin(loc: Loc, args: Vec<Value>) -> Result<Value> {
    unary_f64_op(loc, args, "sin", f64::sin)
}

#[inline]
pub fn math_cos(loc: Loc, args: Vec<Value>) -> Result<Value> {
    unary_f64_op(loc, args, "cos", f64::cos)
}

#[inline]
pub fn math_tan(loc: Loc, args: Vec<Value>) -> Result<Value> {
    unary_f64_op(loc, args, "tan", f64::tan)
}

#[inline]
fn bit_fold(loc: Loc, args: Vec<Value>, fn_name: &str, op: fn(i64, i64) -> i64) -> Result<Value> {
    if args.is_empty() {
        return Err(SelError::Runtime(
            loc,
            format!("{fn_name} requires at least 1 argument"),
        ));
    }
    let mut acc = match args[0] {
        Value::Integer(i) => i,
        ref v => {
            return Err(SelError::TypeError(
                loc,
                format!("{fn_name}: expected integer, found {}", value_type_name(v)),
            ));
        }
    };
    for arg in args.into_iter().skip(1) {
        match arg {
            Value::Integer(i) => acc = op(acc, i),
            ref v => {
                return Err(SelError::TypeError(
                    loc,
                    format!("{fn_name}: expected integer, found {}", value_type_name(v)),
                ));
            }
        }
    }
    Ok(Value::Integer(acc))
}

#[inline]
fn bit_shift(loc: Loc, args: Vec<Value>, fn_name: &str, op: fn(i64, u32) -> i64) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 2,
            actual: args.len(),
        });
    }
    match (&args[0], &args[1]) {
        (Value::Integer(a), Value::Integer(shift)) => {
            if *shift < 0 || *shift >= 64 {
                return Err(SelError::Runtime(
                    loc,
                    format!("{fn_name}: shift out of range: {shift}"),
                ));
            }
            Ok(Value::Integer(op(*a, *shift as u32)))
        }
        _ => Err(SelError::TypeError(
            loc,
            format!("{fn_name}: expected integer arguments"),
        )),
    }
}

pub fn bit_and(loc: Loc, args: Vec<Value>) -> Result<Value> {
    bit_fold(loc, args, "bit-and", |a, b| a & b)
}

pub fn bit_or(loc: Loc, args: Vec<Value>) -> Result<Value> {
    bit_fold(loc, args, "bit-or", |a, b| a | b)
}

pub fn bit_xor(loc: Loc, args: Vec<Value>) -> Result<Value> {
    bit_fold(loc, args, "bit-xor", |a, b| a ^ b)
}

pub fn bit_not(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 1,
            actual: args.len(),
        });
    }
    match args[0] {
        Value::Integer(i) => Ok(Value::Integer(!i)),
        ref v => Err(SelError::TypeError(
            loc,
            format!("bit-not: expected integer, found {}", value_type_name(v)),
        )),
    }
}

pub fn bit_shl(loc: Loc, args: Vec<Value>) -> Result<Value> {
    bit_shift(loc, args, "bit-shl", i64::wrapping_shl)
}

pub fn bit_shr(loc: Loc, args: Vec<Value>) -> Result<Value> {
    bit_shift(loc, args, "bit-shr", i64::wrapping_shr)
}

// String utilities
pub fn string_split(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 2,
            actual: args.len(),
        });
    }
    let s = args[0].to_string_lossy().ok_or_else(|| {
        SelError::TypeError(
            loc,
            format!(
                "string-split: first argument must be string, got {}",
                value_type_name(&args[0])
            ),
        )
    })?;
    let delim = match &args[1] {
        Value::Char(c) => c.to_string(),
        other => other.to_string_lossy().ok_or_else(|| {
            SelError::TypeError(
                loc,
                format!(
                    "string-split: delimiter must be string or char, got {}",
                    value_type_name(other)
                ),
            )
        })?,
    };
    let parts: Vec<Value> = if delim.is_empty() {
        s.chars()
            .map(|c| Value::make_string(&c.to_string()))
            .collect()
    } else {
        s.split(&delim).map(Value::make_string).collect()
    };
    Ok(Value::make_list(parts))
}

pub fn string_join(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 2,
            actual: args.len(),
        });
    }
    let delim = args[1].to_string_lossy().ok_or_else(|| {
        SelError::TypeError(
            loc,
            format!(
                "string-join: delimiter must be string, got {}",
                value_type_name(&args[1])
            ),
        )
    })?;
    let list = match &args[0] {
        Value::List(l) => l,
        Value::Nil => return Ok(Value::make_string("")),
        v => {
            return Err(SelError::TypeError(
                loc,
                format!("string-join: expected list, got {}", value_type_name(v)),
            ));
        }
    };
    let mut pieces = Vec::with_capacity(list.len());
    for item in list.iter() {
        if let Some(s) = item.to_string_lossy() {
            pieces.push(s);
        } else {
            pieces.push(format!("{item}"));
        }
    }
    Ok(Value::make_string(&pieces.join(&delim)))
}

pub fn string_trim(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 1,
            actual: args.len(),
        });
    }
    let s = args[0].to_string_lossy().ok_or_else(|| {
        SelError::TypeError(
            loc,
            format!(
                "string-trim: expected string, got {}",
                value_type_name(&args[0])
            ),
        )
    })?;
    Ok(Value::make_string(s.trim()))
}

pub fn string_replace(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 3 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 3,
            actual: args.len(),
        });
    }
    let s = args[0].to_string_lossy().ok_or_else(|| {
        SelError::TypeError(
            loc,
            format!(
                "string-replace: first argument must be string, got {}",
                value_type_name(&args[0])
            ),
        )
    })?;
    let from = args[1].to_string_lossy().ok_or_else(|| {
        SelError::TypeError(
            loc,
            format!(
                "string-replace: second argument must be string, got {}",
                value_type_name(&args[1])
            ),
        )
    })?;
    let to = args[2].to_string_lossy().ok_or_else(|| {
        SelError::TypeError(
            loc,
            format!(
                "string-replace: third argument must be string, got {}",
                value_type_name(&args[2])
            ),
        )
    })?;
    Ok(Value::make_string(&s.replace(&from, &to)))
}

pub fn string_upcase(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 1,
            actual: args.len(),
        });
    }
    let s = args[0].to_string_lossy().ok_or_else(|| {
        SelError::TypeError(
            loc,
            format!(
                "string-upcase: expected string, got {}",
                value_type_name(&args[0])
            ),
        )
    })?;
    Ok(Value::make_string(&s.to_uppercase()))
}

pub fn string_downcase(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 1,
            actual: args.len(),
        });
    }
    let s = args[0].to_string_lossy().ok_or_else(|| {
        SelError::TypeError(
            loc,
            format!(
                "string-downcase: expected string, got {}",
                value_type_name(&args[0])
            ),
        )
    })?;
    Ok(Value::make_string(&s.to_lowercase()))
}

pub fn to_string(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 1,
            actual: args.len(),
        });
    }
    let val = &args[0];
    if val.to_string_lossy().is_some() {
        Ok(val.clone())
    } else {
        Ok(Value::make_string(&format!("{val}")))
    }
}

pub fn string_format(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.is_empty() {
        return Err(SelError::Runtime(
            loc,
            "format requires at least 1 format string argument".into(),
        ));
    }
    let fmt_str = args[0].to_string_lossy().ok_or_else(|| {
        SelError::TypeError(
            loc,
            format!(
                "format: first argument must be format string, got {}",
                value_type_name(&args[0])
            ),
        )
    })?;
    let mut result = String::with_capacity(fmt_str.len());
    let mut arg_iter = args.iter().skip(1);
    let mut chars = fmt_str.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '{' && chars.peek() == Some(&'}') {
            chars.next(); // consume '}'
            if let Some(arg) = arg_iter.next() {
                if let Some(s) = arg.to_string_lossy() {
                    result.push_str(&s);
                } else {
                    result.push_str(&format!("{arg}"));
                }
            } else {
                return Err(SelError::Runtime(
                    loc,
                    "format: not enough arguments for placeholders".into(),
                ));
            }
            continue;
        }
        result.push(c);
    }
    Ok(Value::make_string(&result))
}

// System & Time primitives
pub fn sys_get_env(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 1,
            actual: args.len(),
        });
    }
    let key = args[0].to_string_lossy().ok_or_else(|| {
        SelError::TypeError(
            loc,
            format!(
                "get-env: expected string variable name, got {}",
                value_type_name(&args[0])
            ),
        )
    })?;
    match crate::os::get_env(&key) {
        Some(val) => Ok(Value::make_string(&val)),
        None => Ok(Value::Nil),
    }
}

pub fn sys_set_env(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 2 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 2,
            actual: args.len(),
        });
    }
    let key = args[0].to_string_lossy().ok_or_else(|| {
        SelError::TypeError(
            loc,
            format!(
                "set-env!: expected string key, got {}",
                value_type_name(&args[0])
            ),
        )
    })?;
    let val = args[1].to_string_lossy().ok_or_else(|| {
        SelError::TypeError(
            loc,
            format!(
                "set-env!: expected string value, got {}",
                value_type_name(&args[1])
            ),
        )
    })?;
    crate::os::set_env(&key, &val);
    Ok(Value::Nil)
}

pub fn time_now_ms(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if !args.is_empty() {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 0,
            actual: args.len(),
        });
    }
    let now = crate::os::time_now_ms()
        .map_err(|e| SelError::Runtime(loc, format!("time-now-ms failed: {e}")))?;
    Ok(Value::Integer(now))
}

pub fn sleep_ms(loc: Loc, args: Vec<Value>) -> Result<Value> {
    if args.len() != 1 {
        return Err(SelError::ArityMismatch {
            loc,
            expected: 1,
            actual: args.len(),
        });
    }
    match args[0] {
        Value::Integer(ms) => {
            if ms > 0 {
                crate::os::sleep(std::time::Duration::from_millis(ms as u64));
            }
            Ok(Value::Nil)
        }
        ref v => Err(SelError::TypeError(
            loc,
            format!("sleep-ms: expected integer, got {}", value_type_name(v)),
        )),
    }
}

pub fn load(env: Rc<RefCell<Env>>) {
    let mut e = env.borrow_mut();
    e.insert(intern("+"), Value::NativeFunction(sum));
    e.insert(intern("-"), Value::NativeFunction(sub));
    e.insert(intern("*"), Value::NativeFunction(mul));
    e.insert(intern("/"), Value::NativeFunction(div));
    e.insert(intern("mod"), Value::NativeFunction(modulo));

    e.insert(intern("not"), Value::NativeFunction(not));
    e.insert(intern("eq?"), Value::NativeFunction(is_equal));
    e.insert(intern("="), Value::NativeFunction(num_eq));
    e.insert(intern("!="), Value::NativeFunction(num_noteq));
    e.insert(intern("<"), Value::NativeFunction(num_lt));
    e.insert(intern(">"), Value::NativeFunction(num_gt));
    e.insert(intern("<="), Value::NativeFunction(num_lte));
    e.insert(intern(">="), Value::NativeFunction(num_gte));

    e.insert(intern("cons"), Value::NativeFunction(cons));
    e.insert(intern("car"), Value::NativeFunction(car));
    e.insert(intern("cdr"), Value::NativeFunction(cdr));
    e.insert(intern("nth"), Value::NativeFunction(nth));
    e.insert(intern("drop"), Value::NativeFunction(drop));
    e.insert(intern("count"), Value::NativeFunction(count));
    e.insert(intern("list"), Value::NativeFunction(list));
    e.insert(intern("empty?"), Value::NativeFunction(empty));
    e.insert(intern("reverse"), Value::NativeFunction(reverse));
    e.insert(intern("append"), Value::NativeFunction(append));
    e.insert(intern("last"), Value::NativeFunction(last));
    e.insert(intern("range"), Value::NativeFunction(range));

    e.insert(intern("rget"), Value::NativeFunction(rget));
    e.insert(intern("rset"), Value::NativeFunction(rset));
    e.insert(intern("rdel"), Value::NativeFunction(rdel));
    e.insert(intern("assoc"), Value::NativeFunction(rset));
    e.insert(intern("dissoc"), Value::NativeFunction(rdel));
    e.insert(intern("rkeys"), Value::NativeFunction(rkeys));
    e.insert(intern("rvals"), Value::NativeFunction(rvals));
    e.insert(intern("rcontains?"), Value::NativeFunction(rcontains));

    e.insert(intern("nil?"), Value::NativeFunction(is_nil));
    e.insert(intern("list?"), Value::NativeFunction(is_list));
    e.insert(intern("number?"), Value::NativeFunction(is_number));
    e.insert(intern("string?"), Value::NativeFunction(is_string));
    e.insert(
        intern("string-contains?"),
        Value::NativeFunction(string_contains),
    );
    e.insert(intern("symbol?"), Value::NativeFunction(is_symbol));
    e.insert(intern("gensym"), Value::NativeFunction(gensym));
    e.insert(intern("function?"), Value::NativeFunction(is_function));
    e.insert(intern("record?"), Value::NativeFunction(is_record));
    e.insert(intern("char?"), Value::NativeFunction(is_char));
    e.insert(
        intern("char->integer"),
        Value::NativeFunction(char_to_integer),
    );
    e.insert(
        intern("integer->char"),
        Value::NativeFunction(integer_to_char),
    );

    e.insert(intern("type-of"), Value::NativeFunction(type_of));

    e.insert(intern("error"), Value::NativeFunction(error));
    e.insert(intern("display"), Value::NativeFunction(display));
    e.insert(intern("println"), Value::NativeFunction(display_newline));
    e.insert(intern("newline"), Value::NativeFunction(newline));

    #[cfg(feature = "ffi")]
    {
        e.insert(intern("ffi-dlopen"), Value::NativeFunction(ffi_dlopen));
        e.insert(intern("ffi-dlsym"), Value::NativeFunction(ffi_dlsym));
        e.insert(intern("ffi-call"), Value::NativeFunction(ffi_call));
    }

    e.insert(intern("system"), Value::NativeFunction(system));
    e.insert(intern("file-system"), Value::NativeFunction(file_system));

    e.insert(intern("abs"), Value::NativeFunction(math_abs));
    e.insert(intern("min"), Value::NativeFunction(math_min));
    e.insert(intern("max"), Value::NativeFunction(math_max));
    e.insert(intern("sqrt"), Value::NativeFunction(math_sqrt));
    e.insert(intern("pow"), Value::NativeFunction(math_pow));
    e.insert(intern("floor"), Value::NativeFunction(math_floor));
    e.insert(intern("ceil"), Value::NativeFunction(math_ceil));
    e.insert(intern("round"), Value::NativeFunction(math_round));
    e.insert(intern("sin"), Value::NativeFunction(math_sin));
    e.insert(intern("cos"), Value::NativeFunction(math_cos));
    e.insert(intern("tan"), Value::NativeFunction(math_tan));
    e.insert(intern("bit-and"), Value::NativeFunction(bit_and));
    e.insert(intern("bit-or"), Value::NativeFunction(bit_or));
    e.insert(intern("bit-xor"), Value::NativeFunction(bit_xor));
    e.insert(intern("bit-not"), Value::NativeFunction(bit_not));
    e.insert(intern("bit-shl"), Value::NativeFunction(bit_shl));
    e.insert(intern("bit-shr"), Value::NativeFunction(bit_shr));

    e.insert(intern("string-split"), Value::NativeFunction(string_split));
    e.insert(intern("string-join"), Value::NativeFunction(string_join));
    e.insert(intern("string-trim"), Value::NativeFunction(string_trim));
    e.insert(
        intern("string-replace"),
        Value::NativeFunction(string_replace),
    );
    e.insert(
        intern("string-upcase"),
        Value::NativeFunction(string_upcase),
    );
    e.insert(
        intern("string-downcase"),
        Value::NativeFunction(string_downcase),
    );
    e.insert(intern("to-string"), Value::NativeFunction(to_string));
    e.insert(intern("format"), Value::NativeFunction(string_format));

    e.insert(intern("get-env"), Value::NativeFunction(sys_get_env));
    e.insert(intern("set-env!"), Value::NativeFunction(sys_set_env));
    e.insert(intern("time-now-ms"), Value::NativeFunction(time_now_ms));
    e.insert(intern("sleep-ms"), Value::NativeFunction(sleep_ms));

    e.insert(intern("co-create"), Value::NativeFunction(co_create));
    e.insert(intern("co-state"), Value::NativeFunction(co_state));
    e.insert(intern("co-dead?"), Value::NativeFunction(co_dead_p));
}

pub fn read_script<P>(script_path: P) -> Result<String>
where
    P: AsRef<Path>,
{
    let mut src =
        crate::os::read_to_string(script_path).map_err(|e| SelError::Internal(e.to_string()))?;
    if src.starts_with("#!") {
        if let Some(newline_idx) = src.find('\n') {
            src = src[newline_idx + 1..].to_string();
        } else {
            src = String::new();
        }
    }
    Ok(src)
}

pub fn load_core_lib() -> Rc<RefCell<Env>> {
    let env = Rc::new(RefCell::new(Env::default()));
    load(env.clone());
    // Load core library if exists
    {
        let core_src = include_str!("core.scm");

        let mut diags = Vec::new();
        let file_id = intern("<core>");

        let asts = parse_all(core_src, file_id, &mut diags);
        if !diags.is_empty() {
            for diag in diags {
                eprintln!("{}", diag);
            }
        }

        if let Err(e) = execute_asts(asts, env.clone()) {
            eprintln!("Error loading core.scm:\n{}", e);
        }
    }
    env
}
