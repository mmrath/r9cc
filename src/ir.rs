// Compile AST to intermediate code that has infinite number of registers.
// Base pointer is always assigned to r0.

use parse::{Node, NodeType};
use token::TokenType;

use std::sync::Mutex;
use std::fmt;
use std::collections::HashMap;

lazy_static!{
    static ref VARS: Mutex<HashMap<String, usize>> = Mutex::new(HashMap::new());

    static ref REGNO: Mutex<usize> = Mutex::new(1);
    static ref STACKSIZE: Mutex<usize> = Mutex::new(0);
    static ref LABEL: Mutex<usize> = Mutex::new(0);
    static ref IRINFO: [IRInfo; 17] = [
        IRInfo::new(IROp::Add, "ADD", IRType::RegReg),
        IRInfo::new(IROp::Sub, "SUB", IRType::RegReg),
        IRInfo::new(IROp::Mul, "MUL", IRType::RegReg),
        IRInfo::new(IROp::Div, "DIV", IRType::RegReg),
        IRInfo::new(IROp::Imm, "MOV", IRType::RegImm),
        IRInfo::new(IROp::SubImm, "SUB", IRType::RegImm),
        IRInfo::new(IROp::Mov, "MOV", IRType::RegReg),
        IRInfo::new(IROp::Label, "", IRType::Label),
        IRInfo::new(IROp::Jmp, "", IRType::Label),
        IRInfo::new(IROp::Unless, "UNLESS", IRType::RegLabel),
        IRInfo::new(IROp::Call(String::new(), 0, [0; 6]), "CALL", IRType::Call),
        IRInfo::new(IROp::Return, "RET", IRType::Reg),
        IRInfo::new(IROp::Load, "LOAD", IRType::RegReg),
        IRInfo::new(IROp::Store, "STORE", IRType::RegReg),
        IRInfo::new(IROp::Kill, "KILL", IRType::Reg),
        IRInfo::new(IROp::SaveArgs, "SAVE_ARGS", IRType::Imm),
        IRInfo::new(IROp::Nop, "NOP", IRType::Noarg),
    ];
}

#[derive(Clone, Debug)]
pub enum IRType {
    Noarg,
    Reg,
    Imm,
    Label,
    RegReg,
    RegImm,
    RegLabel,
    Call,
}

#[derive(Clone, Debug)]
pub struct IRInfo {
    op: IROp,
    name: &'static str,
    pub ty: IRType,
}

impl IRInfo {
    pub fn new(op: IROp, name: &'static str, ty: IRType) -> Self {
        IRInfo {
            op: op,
            name: name,
            ty: ty,
        }
    }
}

impl fmt::Display for IR {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::IRType::*;

        let info = get_irinfo(self);
        let lhs = self.lhs.unwrap();
        match info.ty {
            Label => write!(f, ".L{}=>\n", lhs),
            Imm => write!(f, "{} {}\n", info.name, lhs),
            Reg => write!(f, "{} r{}\n", info.name, lhs),
            RegReg => write!(f, "{} r{}, r{}\n", info.name, lhs, self.rhs.unwrap()),
            RegImm => write!(f, "{} r{}, {}\n", info.name, lhs, self.rhs.unwrap()),
            RegLabel => write!(f, "{} r{}, .L{}\n", info.name, lhs, self.rhs.unwrap()),
            Call => {
                match self.op {
                    IROp::Call(ref name, nargs, args) => {
                        let mut sb: String = format!(", r{} = {}(", lhs, name);
                        for i in 0..nargs {
                            sb.push_str(&format!(", r{}", args[i]));
                        }
                        write!(f, "{}", sb)
                    }
                    _ => unreachable!(),
                }
            }
            Noarg => write!(f, "{}\n", info.name),
        }
    }
}

pub fn dump_ir(fns: &Vec<Function>) {
    for f in fns {
        print!("{}(): \n", f.name);
        for ir in &f.ir {
            print!("  {}", ir);
        }
    }
}

pub fn get_irinfo(ir: &IR) -> IRInfo {
    for info in IRINFO.iter() {
        match ir.op {
            IROp::Call(ref name, nargs, args) => {
                return IRInfo::new(IROp::Call(name.clone(), nargs, args), "CALL", IRType::Call)
            }
            _ => {
                if info.op == ir.op {
                    return info.clone();
                }
            }
        }
    }
    panic!("invalid instruction")
}

#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub ir: Vec<IR>,
    pub stacksize: usize,
}

impl Function {
    fn new(name: String, ir: Vec<IR>, stacksize: usize) -> Self {
        Function {
            name: name,
            // args: args,
            ir: ir,
            stacksize: stacksize,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum IROp {
    Imm,
    Mov,
    Add,
    SubImm,
    Sub,
    Mul,
    Div,
    Return,
    Call(String, usize, [usize; 6]),
    Label,
    Jmp,
    Unless,
    Load,
    Store,
    Kill,
    SaveArgs,
    Nop,
}

impl From<NodeType> for IROp {
    fn from(node_type: NodeType) -> Self {
        match node_type {
            NodeType::BinOp(op, _, _) => Self::from(op),
            e => panic!("cannot convert: {:?}", e),
        }
    }
}

impl From<TokenType> for IROp {
    fn from(token_type: TokenType) -> Self {
        match token_type {
            TokenType::Plus => IROp::Add,
            TokenType::Minus => IROp::Sub,
            TokenType::Mul => IROp::Mul,
            TokenType::Div => IROp::Div,
            e => panic!("cannot convert: {:?}", e),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IR {
    pub op: IROp,
    pub lhs: Option<usize>,
    pub rhs: Option<usize>,
}

impl IR {
    fn new(op: IROp, lhs: Option<usize>, rhs: Option<usize>) -> Self {
        Self {
            op: op,
            lhs: lhs,
            rhs: rhs,
        }
    }
}

fn gen_lval(code: &mut Vec<IR>, node: Node) -> Option<usize> {
    match node.ty {
        NodeType::Ident(name) => {
            if VARS.lock().unwrap().get(&name).is_none() {
                VARS.lock().unwrap().insert(
                    name.clone(),
                    *STACKSIZE.lock().unwrap(),
                );
                *STACKSIZE.lock().unwrap() += 8;
            }
            let r = Some(*REGNO.lock().unwrap());
            *REGNO.lock().unwrap() += 1;
            let off = *VARS.lock().unwrap().get(&name).unwrap();
            code.push(IR::new(IROp::Mov, r, Some(0)));
            code.push(IR::new(IROp::SubImm, r, Some(off)));
            return r;
        }
        _ => panic!("not an lvalue"),
    }
}

fn gen_expr(code: &mut Vec<IR>, node: Node) -> Option<usize> {
    match node.ty {
        NodeType::Num(val) => {
            let r = Some(*REGNO.lock().unwrap());
            *REGNO.lock().unwrap() += 1;
            code.push(IR::new(IROp::Imm, r, Some(val as usize)));
            return r;
        }
        NodeType::Ident(_) => {
            let r = gen_lval(code, node);
            code.push(IR::new(IROp::Load, r, r));
            return r;
        }
        NodeType::Call(name, args) => {
            let mut args_ir: [usize; 6] = [0; 6];
            for i in 0..args.len() {
                args_ir[i] = gen_expr(code, args[i].clone()).unwrap();
            }

            let r = Some(*REGNO.lock().unwrap());
            *REGNO.lock().unwrap() += 1;

            code.push(IR::new(IROp::Call(name, args.len(), args_ir), r, None));

            for i in 0..args.len() {
                code.push(IR::new(IROp::Kill, Some(args_ir[i]), None));
            }
            return r;
        }
        NodeType::BinOp(op, lhs, rhs) => {
            match op {
                TokenType::Equal => {
                    let rhs = gen_expr(code, *rhs);
                    let lhs = gen_lval(code, *lhs);
                    code.push(IR::new(IROp::Store, lhs, rhs));
                    code.push(IR::new(IROp::Kill, rhs, None));
                    return lhs;
                }
                _ => {
                    let lhs = gen_expr(code, *lhs);
                    let rhs = gen_expr(code, *rhs);

                    code.push(IR::new(IROp::from(op), lhs, rhs));
                    code.push(IR::new(IROp::Kill, rhs, None));
                    return lhs;
                }
            }
        }
        _ => unreachable!(),
    }
}

fn gen_stmt(code: &mut Vec<IR>, node: Node) {
    match node.ty {
        NodeType::If(cond, then, els_may) => {
            let r = gen_expr(code, *cond);
            let x = Some(*LABEL.lock().unwrap());
            *LABEL.lock().unwrap() += 1;
            code.push(IR::new(IROp::Unless, r, x));
            code.push(IR::new(IROp::Kill, r, None));
            gen_stmt(code, *then);

            if let Some(els) = els_may {
                let y = Some(*LABEL.lock().unwrap());
                *LABEL.lock().unwrap() += 1;
                code.push(IR::new(IROp::Jmp, y, None));
                code.push(IR::new(IROp::Label, x, None));
                gen_stmt(code, *els);
                code.push(IR::new(IROp::Label, y, None));
                return;
            } else {
                code.push(IR::new(IROp::Label, x, None));
                return;
            }
        }
        NodeType::Return(expr) => {
            let r = gen_expr(code, *expr);
            code.push(IR::new(IROp::Return, r, None));
            code.push(IR::new(IROp::Kill, r, None));
        }
        NodeType::ExprStmt(expr) => {
            let r = gen_expr(code, *expr);
            code.push(IR::new(IROp::Kill, r, None));
        }
        NodeType::CompStmt(stmts) => {
            for n in stmts {
                gen_stmt(code, n);
            }
        }
        e => panic!("unknown node: {:?}", e),
    }
}

fn gen_args(code: &mut Vec<IR>, nodes: Vec<Node>) {
    if nodes.len() == 0 {
        return;
    }

    code.push(IR::new(IROp::SaveArgs, Some(nodes.len()), None));

    for node in nodes {
        match node.ty {
            NodeType::Ident(name) => {
                *STACKSIZE.lock().unwrap() += 8;
                VARS.lock().unwrap().insert(
                    name.clone(),
                    *STACKSIZE.lock().unwrap(),
                );
            }
            _ => panic!("bad parameter"),
        }
    }
}

pub fn gen_ir(nodes: Vec<Node>) -> Vec<Function> {
    let mut v = vec![];
    for node in nodes {
        match node.ty {
            NodeType::Func(name, args, body) => {
                let mut code = vec![];
                *VARS.lock().unwrap() = HashMap::new();

                *REGNO.lock().unwrap() = 1;
                *STACKSIZE.lock().unwrap() = 0;

                gen_args(&mut code, args);
                gen_stmt(&mut code, *body);

                v.push(Function::new(name, code, *STACKSIZE.lock().unwrap()));
            }
            _ => panic!("parse error."),
        }
    }
    v
}
