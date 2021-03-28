pub mod decode;
pub mod display;
pub mod operations;

use operations::{AddI, Call, FStop, PopCopy, PushCopy, PushI, ResV, Ret};

#[derive(Clone, Debug, PartialEq)]
pub enum Instruction {
    /// Pushes a constant integer on the stack
    ///
    /// ```none
    /// push(v)
    /// ```
    PushI(PushI),

    /// Pops two integers from the stack, add them toghether, pushes the result
    ///
    /// ```none
    /// a = s.pop()
    /// b = s.pop()
    /// push(a + b)
    /// ```
    AddI(AddI),

    /// Stops the program, with s[0] as return value.
    FStop(FStop),

    /// Copies a value at a given index, pushes it on top of the stack.
    ///
    /// ```none
    /// a = get(idx)
    /// push(a)
    /// ```
    PushCopy(PushCopy),

    /// Pushes the current instruction pointer on the stack, sets the
    /// instruction pointer to the specified address.
    ///
    /// ```none
    /// push(ip)
    /// ip = ptr
    /// ```
    Call(Call),

    /// Sets the instruction pointer to a value in the stack, shrinks the
    /// stack by a specific amount.
    ///
    /// ```none
    /// ip = get(pointer)
    /// shrink(len(stack) - return)
    /// ```
    Ret(Ret),

    /// Pushes a constant amount of zeros in the stack.
    ///
    /// ```none
    /// for _ in 0..n {
    ///     push(0)
    /// }
    /// ```
    ResV(ResV),

    /// Pops a value from the stack and copies it at a given stack index.
    ///
    /// ```none
    /// tmp = peek()
    /// set(index, tmp)
    /// pop()
    /// ```
    PopCopy(PopCopy),
}

impl Instruction {
    pub fn push_i(i: i32) -> Instruction {
        PushI(i).into()
    }

    pub fn add_i() -> Instruction {
        AddI.into()
    }

    pub fn f_stop() -> Instruction {
        FStop.into()
    }

    pub fn push_cpy(idx: u16) -> Instruction {
        PushCopy(idx).into()
    }

    pub fn call(ptr: u32) -> Instruction {
        Call(ptr).into()
    }

    pub fn ret(ip_offset: u16, shrink_offset: u16) -> Instruction {
        Ret { shrink_offset, ip_offset }.into()
    }

    pub fn res_v(idx: u16) -> Instruction {
        ResV(idx).into()
    }

    pub fn pop_cpy(idx: u16) -> Instruction {
        PopCopy(idx).into()
    }
}

macro_rules! impl_from_operation {
    ($( $operation:ident ),* $(,)?) => {
        $(
            impl From<$operation> for Instruction {
                fn from(op: $operation) -> Instruction {
                    Instruction::$operation(op)
                }
            }
        )*
    };
}

impl_from_operation! { PushI, AddI, FStop, PushCopy, Call, Ret, ResV, PopCopy }
