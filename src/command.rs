use nom::{
    branch::{alt, permutation},
    bytes::complete::{is_a, tag_no_case},
    character::complete::{digit1, multispace0},
    combinator::{map_res, opt, value},
    sequence::preceded,
    IResult, Parser,
};

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct UnsignSet {
    pub x: Option<u32>,
    pub y: Option<u32>,
    pub z: Option<u32>,
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Set {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub z: Option<i32>,
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Cmd {
    Goto(Set),
    Move(Set),
    SpeedMin(UnsignSet),
    SpeedMax(UnsignSet),
    SpeedAccel(UnsignSet),
    StepPerMM(UnsignSet),
    AddPos(Set),
    DelPos(u32),
    ListPos,
    Start,
    Stop,
    Home,
    Help,
}
fn parse_i32(input: &str) -> IResult<&str, i32> {
    preceded(
        multispace0,
        alt((
            map_res(preceded(is_a("-"), digit1), |s: &str| {
                s.parse().map(|n: i32| -n)
            }),
            map_res(digit1, |s: &str| s.parse()),
        )),
    )
    .parse(input)
}
fn parse_u32(input: &str) -> IResult<&str, u32> {
    preceded(
        multispace0,
        alt((
            map_res(preceded(is_a("+"), digit1), |s: &str| s.parse()),
            map_res(digit1, |s: &str| s.parse()),
        )),
    )
    .parse(input)
}

fn parse_ix(input: &str) -> IResult<&str, i32> {
    preceded(multispace0, preceded(tag_no_case("x"), parse_i32)).parse(input)
}
fn parse_iy(input: &str) -> IResult<&str, i32> {
    preceded(multispace0, preceded(tag_no_case("y"), parse_i32)).parse(input)
}
fn parse_iz(input: &str) -> IResult<&str, i32> {
    preceded(multispace0, preceded(tag_no_case("z"), parse_i32)).parse(input)
}
fn parse_ux(input: &str) -> IResult<&str, u32> {
    preceded(multispace0, preceded(tag_no_case("x"), parse_u32)).parse(input)
}
fn parse_uy(input: &str) -> IResult<&str, u32> {
    preceded(multispace0, preceded(tag_no_case("y"), parse_u32)).parse(input)
}
fn parse_uz(input: &str) -> IResult<&str, u32> {
    preceded(multispace0, preceded(tag_no_case("z"), parse_u32)).parse(input)
}

fn parse_position(input: &str) -> IResult<&str, Set> {
    permutation((opt(parse_ix), opt(parse_iy), opt(parse_iz)))
        .map(|(x, y, z)| Set { x, y, z })
        .parse(input)
}
fn parse_value(input: &str) -> IResult<&str, UnsignSet> {
    permutation((opt(parse_ux), opt(parse_uy), opt(parse_uz)))
        .map(|(x, y, z)| UnsignSet { x, y, z })
        .parse(input)
}
pub fn parse_cmd(input: &str) -> IResult<&str, Cmd> {
    alt((
        preceded(tag_no_case("goto"), parse_position).map(Cmd::Goto),
        preceded(tag_no_case("move"), parse_position).map(Cmd::Move),
        preceded(tag_no_case("speed max"), parse_value).map(Cmd::SpeedMin),
        preceded(tag_no_case("speed min"), parse_value).map(Cmd::SpeedMax),
        preceded(tag_no_case("speed acc"), parse_value).map(Cmd::SpeedAccel),
        preceded(tag_no_case("step_per_mm"), parse_value).map(Cmd::StepPerMM),
        preceded(tag_no_case("add pos"), parse_position).map(Cmd::AddPos),
        preceded(tag_no_case("del pos"), parse_u32).map(Cmd::DelPos),
        value(Cmd::ListPos, tag_no_case("list pos")),
        value(Cmd::Start, tag_no_case("start")),
        value(Cmd::Stop, tag_no_case("stop")),
        value(Cmd::Home, tag_no_case("home")),
        value(Cmd::Help, tag_no_case("help")),
    ))
    .parse(input)
}
