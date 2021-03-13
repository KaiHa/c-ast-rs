use anyhow::Result;
use lang_c::ast::*;
use lang_c::driver::{parse, Config};
use lang_c::span::Span;
use lang_c::visit;
use lang_c::visit::Visit;
use log;
use simple_logger::SimpleLogger;
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use structopt::StructOpt;
use structopt_flags::LogLevel;

fn main() -> Result<()> {
    let opt = Opt::from_args();
    SimpleLogger::new()
        .with_level(opt.verbose.get_level_filter())
        .init()?;
    let config = Config::default();
    let unit = parse(&config, opt.file)?.unit;
    let strcts = &mut HashMap::new();
    let vals = &mut HashMap::new();
    let mut myp = MyVisitor::new(strcts, vals);
    myp.visit_translation_unit(&unit);
    println!("Struct-Types: {:#?}", &myp.struct_types);
    println!("");
    for v in myp.values.values() {
        println!("{}", v);
    }
    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "c-ast", about = "parce c file and print interesting structs")]
struct Opt {
    #[structopt(flatten)]
    verbose: structopt_flags::QuietVerbose,
    #[structopt(
        short,
        long,
        parse(try_from_str = parse_path),
        default_value = "./main.c"
    )]
    file: PathBuf,
}

fn parse_path(s: &str) -> Result<PathBuf> {
    let s = shellexpand::full(s)?;
    Ok(PathBuf::from(String::from(s)))
}


#[derive(Debug)]
pub struct MyStructType {
    name: String,
    fields: Vec<String>,
}

impl MyStructType {
    fn new(name: &str) -> MyStructType {
        MyStructType {
            name: String::from(name),
            fields: Vec::new(),
        }
    }
}


#[derive(Debug)]
pub enum MyValue {
    Struct(MyStruct),
    Scalar { name: String, value: MyExpression },
}

impl MyValue {
    fn new_struct(typ: &str, name: &str) -> MyValue {
        MyValue::Struct(MyStruct {
            typ: String::from(typ),
            name: String::from(name),
            values: Vec::new(),
        })
    }

    fn new_scalar(name: &str, value: MyExpression) -> MyValue {
        MyValue::Scalar {
            name: String::from(name),
            value: value,
        }
    }
}

impl fmt::Display for MyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MyValue::Struct(s) => writeln!(f, "{}", s)?,
            MyValue::Scalar {name: n, value: v} => writeln!(f, "{} = {:?}", n, v)?,
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct MyStruct {
    typ: String,
    name: String,
    values: Vec<(String, MyExpression)>
}

impl fmt::Display for MyStruct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "struct {} {}", self.typ, self.name)?;
        for (n, e) in &self.values {
            writeln!(f, "  .{} = {:?}", n, e)?;
        }
        Ok(())
    }
}


#[derive(Debug)]
pub enum MyExpression {
    Integer(String),
    Float(String),
    String(String),
    StringLiteral(Vec<String>),
    Other(String),
}


pub struct MyVisitor<'a> {
    cur_struct: Option<String>,
    struct_types: &'a mut HashMap<String, MyStructType>,
    values: &'a mut HashMap<(Option<String>, String), MyValue>,
}

impl<'a> MyVisitor<'a> {
    pub fn new(
        s: &'a mut HashMap<String, MyStructType>,
        v: &'a mut HashMap<(Option<String>, String), MyValue>,
    ) -> MyVisitor<'a> {
        MyVisitor {
            cur_struct: None,
            struct_types: s,
            values: v,
        }
    }
}

impl<'ast, 'a> Visit<'ast> for MyVisitor<'a> {
    fn visit_struct_type(&mut self, n: &'ast StructType, span: &'ast Span) {
        if let Some(ref id) = n.identifier {
            self.cur_struct = Some(String::from(&id.node.name));
        }
        visit::visit_struct_type(self, n, span);
    }

    fn visit_struct_field(&mut self, n: &'ast StructField, _: &'ast Span) {
        if let Some(struct_name) = self.cur_struct.as_ref() {
            for declarator in &n.declarators {
                if let Some(x) = &declarator.node.declarator {
                    match &x.node.kind.node {
                        DeclaratorKind::Identifier(y) => {
                            self.struct_types
                                .entry(String::from(struct_name))
                                .or_insert(MyStructType::new(struct_name))
                                .fields
                                .push(String::from(&y.node.name));
                        }
                        _ => (),
                    }
                }
            }
        } else {
            log::warn!("I visit struct fields but I don't know in which struct I am!\n {:#?}", n)
        }
    }

    fn visit_init_declarator(&mut self, n: &'ast InitDeclarator, _: &'ast Span) {
        match &n.declarator.node.kind.node {
            DeclaratorKind::Identifier(x) => {
                if let Some(ini) = &n.initializer {
                    match &ini.node {
                        Initializer::List(xs) => {
                            if let Some(struct_name) = &self.cur_struct {
                                if let MyValue::Struct(mst) = self.values
                                    .entry((self.cur_struct.clone(), String::from(&x.node.name)))
                                    .or_insert(MyValue::new_struct(struct_name, &x.node.name)) {
                                        if let Some (stype) = self.struct_types.get(self.cur_struct.as_ref().unwrap()) {
                                            for (x, fname) in xs.into_iter().zip(stype.fields.clone()) {
                                                fill(&mut mst.values, &fname, &x.node.initializer.node);
                                            }
                                        } else {
                                            panic!("Struct type '{}' not found", self.cur_struct.as_ref().unwrap())
                                        }
                                    }
                            }
                        }
                        Initializer::Expression(e) => {
                            self.values
                                .entry((self.cur_struct.clone(), String::from(&x.node.name)))
                                .or_insert(MyValue::new_scalar(&x.node.name, transform(&e.node)));
                            ()
                        },
                    }
                }
            }
            x => panic!("Expected an identifier but got {:?}", x),
        }
    }
}


fn fill(acc: &mut Vec<(String, MyExpression)>, fname: &str, ini: &Initializer) {
    match &ini {
        Initializer::Expression(e) => acc.push((String::from(fname), transform(&e.node))),
        Initializer::List(ls) => for l in ls { fill(acc, fname, &l.node.initializer.node) },
    }
}


fn transform(expr: &Expression) -> MyExpression {
    match expr {
        Expression::Constant(a) => match &a.node {
            Constant::Integer(b) => MyExpression::Integer(String::from(b.number.as_ref())),
            Constant::Float(b) => MyExpression::Float(String::from(b.number.as_ref())),
            Constant::Character(b) => MyExpression::String(String::from(b)),
        },
        Expression::StringLiteral(a) => MyExpression::StringLiteral(a.node.clone()),
        a => MyExpression::Other(format!("{:?}", a)),
    }
}
