use crate::utils::number::build_number;
use std::mem;

use crate::vm::local::Local;
use crate::vm::data::{Data, Tagged};
use crate::vm::stack::{Stack, Item};
use crate::pipeline::bytecode::{Chunk, Opcode};

// I'm not sure if a garbage collector is necessary
// Rust makes sure there are no memory leaks
// and all non-returned values are freed when they go out of scope as per design
// also, I'm cloning everything all over the place
// I need to either implement resiliant-whatever datastructures (like FP)
// or get my act together and do pass by object reference or something

#[derive(Debug)]
pub struct VM {
    chunk: Chunk,
    stack: Stack,
    ip:    usize,
}

type RunResult = Option<()>;

// NOTE: use Opcode::same and Opcode.to_byte() rather than actual bytes
// Don't worry, the compiler *should* get rid of this overhead and just use bytes

// this impl contains initialization, helper functions, and the core interpreter loop
// the below impl contains opcode implementations
impl VM {
    pub fn init() -> VM {
        VM {
            chunk: Chunk::empty(),
            stack: vec![Item::Frame],
            ip:    0,
        }
    }

    fn next(&mut self)                   { self.ip += 1; }
    fn done(&mut self)      -> RunResult { self.next(); Some(()) }
    fn peek_byte(&mut self) -> u8        { self.chunk.code[self.ip] }
    fn next_byte(&mut self) -> u8        { self.done(); self.peek_byte() }

    fn next_number(&mut self) -> usize {
        self.next();
        let remaining      = self.chunk.code[self.ip..].to_vec();
        let (index, eaten) = build_number(remaining);
        self.ip += eaten - 1; // ip left on next op
        return index;
    }

    fn find_local(&mut self, local: &Local) -> Option<usize> {
        for (index, item) in self.stack.iter().enumerate().rev() {
            match item {
                Item::Local { local: l, .. } => if local == l { return Some(index); },
                Item::Frame                  => break,
                Item::Data(_)                => (),
            }
        }

        return None;
    }

    fn local_index(&mut self) -> (Local, Option<usize>) {
        let local_index = self.next_number();
        let local       = self.chunk.locals[local_index].clone();
        let index       = self.find_local(&local);

        return (local, index);
    }

    // core interpreter loop

    fn step(&mut self) -> RunResult {
        let op_code = Opcode::from_byte(self.peek_byte());

        match op_code {
            Opcode::Con   => self.con(),
            Opcode::Save  => self.save(),
            Opcode::Load  => self.load(),
            Opcode::Clear => self.clear(),
        }
    }

    fn run(&mut self, chunk: Chunk) -> RunResult {
        // cache current state, load new bytecode
        let old_chunk = mem::replace(&mut self.chunk, chunk);

        while self.ip < self.chunk.code.len() {
            self.step();
            println!("{:?}", self.stack);
        }

        // return current state
        mem::drop(mem::replace(&mut self.chunk, old_chunk));

        // nothing went wrong!
        return Some(());
    }
}

// TODO: there are a lot of optimizations that can be made
// i'll list a few here:
// - searching the stack for variables
//   A global Hash-table has significantly less overhead for function calls
// - cloning the heck out of everything - useless copies
// - replace some panics with runresults
impl VM {
    fn con(&mut self) -> RunResult {
        // get the constant index
        let index = self.next_number();

        self.stack.push(Item::Data(Tagged::from(self.chunk.constants[index].clone())));
        self.done()
    }

    fn save(&mut self) -> RunResult {
        let data = match self.stack.pop()? { Item::Data(d) => d.deref(), _ => panic!("Expected data") };
        let (local, index) = self.local_index();

        // NOTE: Does it make a copy or does it make a reference?
        // It makes a copy of the data
        match index {
            // It's been declared
            Some(i) => mem::drop(
                mem::replace(
                    &mut self.stack[i],
                    Item::Local { local, data },
                )
            ),
            // It hasn't been declared
            None => self.stack.push(Item::Local { local, data }),
        }

        self.done()
    }

    fn load(&mut self) -> RunResult {
        let (_, index) = self.local_index();

        match index {
            Some(i) => {
                if let Item::Local { data, .. } = &self.stack[i] {
                    let data = Item::Data(Tagged::from(data.clone()));
                    self.stack.push(data);
                }
            },
            None => panic!("Local not found on stack!"), // TODO: make it a Passerine error
        }

        self.done()
    }

    fn clear(&mut self) -> RunResult {
        loop {
            match self.stack.pop()? {
                Item::Data(_) => (),
                l             => { self.stack.push(l); break; },
            }
        }

        self.done()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::compiler::{
        parse::parse,
        lex::lex,
        gen::gen,
    };

    #[test]
    fn init_run() {
        // TODO: check @ each step, write more tests
        let chunk = gen(parse(lex(
            "boop = 37.201; true; dhuiew = true; boop"
        ).unwrap()).unwrap());

        let mut vm = VM::init();

        match vm.run(chunk) {
            Some(_) => (),
            None    => panic!("VM threw error"),
        }
    }

    #[test]
    fn block_expression() {
        let chunk = gen(parse(lex(
            "boop = true; heck = { x = boop; x }; heck"
        ).unwrap()).unwrap());

        println!("{:#?}", chunk);

        let mut vm = VM::init();

        match vm.run(chunk) {
            Some(_) => (),
            None    => panic!("VM threw error"),
        }

        if let Some(Item::Data(t)) = vm.stack.pop() {
            match t.deref() {
                Data::Boolean(true) => (),
                _                   => panic!("Expected true value")
            }
        } else {
            panic!("Expected data on top of stack")
        }
    }
}
