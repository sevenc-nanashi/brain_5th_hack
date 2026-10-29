use alloc::{string::String, vec::Vec};
use core::f64::consts::{E, PI};

pub const MAX_EXPRESSION: usize = 512;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Angle {
    Deg,
    Rad,
}
impl Angle {
    pub fn label(self) -> &'static str {
        match self {
            Self::Deg => "DEG",
            Self::Rad => "RAD",
        }
    }
    pub fn toggle(self) -> Self {
        match self {
            Self::Deg => Self::Rad,
            Self::Rad => Self::Deg,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Error {
    pub position: usize,
    pub message: &'static str,
}
#[derive(Clone, Copy, Debug)]
enum Token {
    Number(f64),
    Name(&'static str),
    Plus,
    Minus,
    Mul,
    Div,
    Power,
    Open,
    Close,
    End,
}
#[derive(Clone, Copy)]
struct Located {
    token: Token,
    position: usize,
}
fn error(position: usize, message: &'static str) -> Error {
    Error { position, message }
}

/// Returns the value and the expression with trailing parentheses completed.
pub fn evaluate(input: &str, angle: Angle, ans: Option<f64>) -> Result<(f64, String), Error> {
    if input.len() > MAX_EXPRESSION {
        return Err(error(MAX_EXPRESSION, "Expression too long"));
    }
    let bytes = input.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    let mut balance = 0usize;
    while i < bytes.len() {
        let start = i;
        let token = match bytes[i] {
            b' ' | b'\t' | b'\n' => {
                i += 1;
                continue;
            }
            b'0'..=b'9' | b'.' => {
                i += 1;
                while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                    i += 1;
                }
                // Uppercase E denotes scientific notation; lowercase e is Euler's constant.
                if i < bytes.len() && bytes[i] == b'E' {
                    i += 1;
                    if i < bytes.len() && matches!(bytes[i], b'+' | b'-') {
                        i += 1;
                    }
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                let number: f64 = input[start..i]
                    .parse()
                    .map_err(|_| error(start, "Invalid number"))?;
                if !number.is_finite() {
                    return Err(error(start, "Number out of range"));
                }
                tokens.push(Located {
                    token: Token::Number(number),
                    position: start,
                });
                continue;
            }
            b'a'..=b'z' | b'A'..=b'Z' => {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let name = match &input[start..i] {
                    "sin" => "sin",
                    "cos" => "cos",
                    "tan" => "tan",
                    "asin" => "asin",
                    "acos" => "acos",
                    "atan" => "atan",
                    "ln" => "ln",
                    "log" => "log",
                    "sqrt" => "sqrt",
                    "exp" => "exp",
                    "pi" => "pi",
                    "e" => "e",
                    "Ans" | "ans" => "Ans",
                    _ => return Err(error(start, "Unknown name")),
                };
                tokens.push(Located {
                    token: Token::Name(name),
                    position: start,
                });
                continue;
            }
            b'+' => Token::Plus,
            b'-' => Token::Minus,
            b'*' => Token::Mul,
            b'/' => Token::Div,
            b'^' => Token::Power,
            b'(' => {
                balance += 1;
                if balance > 32 {
                    return Err(error(i, "Nesting limit"));
                }
                Token::Open
            }
            b')' => {
                if balance == 0 {
                    return Err(error(i, "Unexpected )"));
                }
                balance -= 1;
                Token::Close
            }
            _ => return Err(error(i, "Invalid character")),
        };
        tokens.push(Located {
            token,
            position: start,
        });
        i += 1;
    }
    if input.len() + balance > MAX_EXPRESSION {
        return Err(error(input.len(), "Expression too long"));
    }
    let mut completed = String::from(input);
    for _ in 0..balance {
        completed.push(')');
        tokens.push(Located {
            token: Token::Close,
            position: input.len(),
        });
    }
    tokens.push(Located {
        token: Token::End,
        position: input.len(),
    });
    let mut parser = Parser {
        tokens: &tokens,
        index: 0,
        angle,
        ans,
    };
    let result = parser.expression(0, 0)?;
    if !matches!(parser.current().token, Token::End) {
        return Err(error(parser.current().position, "Unexpected token"));
    }
    Ok((result, completed))
}

struct Parser<'a> {
    tokens: &'a [Located],
    index: usize,
    angle: Angle,
    ans: Option<f64>,
}
impl Parser<'_> {
    fn current(&self) -> Located {
        self.tokens[self.index]
    }
    fn expression(&mut self, minimum: u8, depth: usize) -> Result<f64, Error> {
        if depth > 32 {
            return Err(error(self.current().position, "Nesting limit"));
        }
        let current = self.current();
        self.index += 1;
        let mut left = match current.token {
            Token::Number(number) => number,
            Token::Plus => self.expression(3, depth + 1)?,
            Token::Minus => -self.expression(3, depth + 1)?,
            Token::Open => {
                let value = self.expression(0, depth + 1)?;
                self.close()?;
                value
            }
            Token::Name("pi") => PI,
            Token::Name("e") => E,
            Token::Name("Ans") => self
                .ans
                .ok_or(error(current.position, "No previous result"))?,
            Token::Name(name) => {
                if !matches!(self.current().token, Token::Open) {
                    return Err(error(self.current().position, "Expected ("));
                }
                self.index += 1;
                let value = self.expression(0, depth + 1)?;
                self.close()?;
                self.function(name, value, current.position)?
            }
            _ => return Err(error(current.position, "Expected value")),
        };
        loop {
            let operator = self.current();
            let (binding, right_binding, implicit) = match operator.token {
                Token::Plus | Token::Minus => (1, 2, false),
                Token::Mul | Token::Div => (2, 3, false),
                Token::Power => (4, 4, false),
                Token::Name(_) | Token::Open => (2, 3, true),
                Token::Number(_)
                    if matches!(
                        self.tokens[self.index - 1].token,
                        Token::Close | Token::Name(_)
                    ) =>
                {
                    (2, 3, true)
                }
                _ => break,
            };
            if binding < minimum {
                break;
            }
            if !implicit {
                self.index += 1;
            }
            let right = self.expression(right_binding, depth + 1)?;
            left = if implicit {
                left * right
            } else {
                match operator.token {
                    Token::Plus => left + right,
                    Token::Minus => left - right,
                    Token::Mul => left * right,
                    Token::Div => {
                        if right == 0.0 {
                            return Err(error(operator.position, "Division by zero"));
                        }
                        left / right
                    }
                    Token::Power => {
                        if (left == 0.0 && right <= 0.0)
                            || (left < 0.0 && libm::trunc(right) != right)
                        {
                            return Err(error(operator.position, "Power domain error"));
                        }
                        libm::pow(left, right)
                    }
                    _ => unreachable!(),
                }
            };
            if !left.is_finite() {
                return Err(error(operator.position, "Result out of range"));
            }
        }
        Ok(left)
    }
    fn close(&mut self) -> Result<(), Error> {
        if !matches!(self.current().token, Token::Close) {
            return Err(error(self.current().position, "Expected )"));
        }
        self.index += 1;
        Ok(())
    }
    fn function(&self, name: &str, value: f64, position: usize) -> Result<f64, Error> {
        let radians = if self.angle == Angle::Deg {
            (value % 360.0) * (PI / 180.0)
        } else {
            value
        };
        let inverse = |x| {
            if self.angle == Angle::Deg {
                x * (180.0 / PI)
            } else {
                x
            }
        };
        let domain = || error(position, "Function domain error");
        let result = match name {
            "sin" => libm::sin(radians),
            "cos" => libm::cos(radians),
            "tan" => {
                if (self.angle == Angle::Deg && value % 180.0 == 90.0)
                    || (self.angle == Angle::Deg && value % 180.0 == -90.0)
                    || (self.angle == Angle::Rad && (value % PI).abs() == PI / 2.0)
                {
                    return Err(domain());
                }
                libm::tan(radians)
            }
            "asin" => {
                if value.abs() > 1.0 {
                    return Err(domain());
                }
                inverse(libm::asin(value))
            }
            "acos" => {
                if value.abs() > 1.0 {
                    return Err(domain());
                }
                inverse(libm::acos(value))
            }
            "atan" => inverse(libm::atan(value)),
            "sqrt" => {
                if value < 0.0 {
                    return Err(domain());
                }
                libm::sqrt(value)
            }
            "ln" => {
                if value <= 0.0 {
                    return Err(domain());
                }
                libm::log(value)
            }
            "log" => {
                if value <= 0.0 {
                    return Err(domain());
                }
                libm::log10(value)
            }
            "exp" => libm::exp(value),
            _ => unreachable!(),
        };
        if result.is_finite() {
            Ok(result)
        } else {
            Err(error(position, "Result out of range"))
        }
    }
}
