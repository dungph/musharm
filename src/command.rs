use defmt::info;
use nom::{
    branch::{alt, permutation},
    bytes::complete::{is_a, tag_no_case},
    character::complete::{digit1, multispace0},
    combinator::{map_res, opt, value},
    sequence::{preceded, tuple},
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
    PumpOn,
    PumpOff,
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
fn parse_3(input: &str) -> IResult<&str, Set> {
    permutation((parse_ix, parse_iy, parse_iz))
        .or(permutation((parse_ix, parse_iz, parse_iy)).map(|(x, z, y)| (x, y, z)))
        .or(permutation((parse_iy, parse_ix, parse_iz)).map(|(y, x, z)| (x, y, z)))
        .or(permutation((parse_iy, parse_iz, parse_ix)).map(|(y, z, x)| (x, y, z)))
        .or(permutation((parse_iz, parse_iy, parse_ix)).map(|(z, y, x)| (x, y, z)))
        .or(permutation((parse_iz, parse_ix, parse_iy)).map(|(z, x, y)| (x, y, z)))
        .or(tuple((parse_i32, parse_i32, parse_i32)))
        .map(|(x, y, z)| Set {
            x: Some(x),
            y: Some(y),
            z: Some(z),
        })
        .parse(input)
}

fn parse_2(input: &str) -> IResult<&str, Set> {
    permutation((parse_ix, parse_iy))
        .map(|(x, y)| (Some(x), Some(y), None))
        .or(permutation((parse_iy, parse_ix)).map(|(y, x)| (Some(x), Some(y), None)))
        .or(permutation((parse_iy, parse_iz)).map(|(y, z)| (None, Some(y), Some(z))))
        .or(permutation((parse_iz, parse_iy)).map(|(z, y)| (None, Some(y), Some(z))))
        .or(permutation((parse_iz, parse_ix)).map(|(z, x)| (Some(x), None, Some(z))))
        .or(permutation((parse_ix, parse_iz)).map(|(x, z)| (Some(x), None, Some(z))))
        .map(|(x, y, z)| Set { x, y, z })
        .parse(input)
}
fn parse_1(input: &str) -> IResult<&str, Set> {
    parse_ix
        .map(|x| (Some(x), None, None))
        .or(parse_iy.map(|x| (None, Some(x), None)))
        .or(parse_iz.map(|x| (None, None, Some(x))))
        .map(|(x, y, z)| Set { x, y, z })
        .parse(input)
}

fn parse_set(input: &str) -> IResult<&str, Set> {
    parse_3
        .or(parse_2)
        .or(parse_1)
        .parse(input)
        .map(|(s, set)| {
            info!("parse set {} {} {}", set.x, set.y, set.z);
            (s, set)
        })
}

fn parse_set_unsigned(input: &str) -> IResult<&str, UnsignSet> {
    fn lt0(x: i32) -> bool {
        x < 0
    }
    map_res(parse_3.or(parse_2).or(parse_1), |set| {
        if set.x.is_some_and(lt0) || set.y.is_some_and(lt0) || set.z.is_some_and(lt0) {
            Err(())
        } else {
            Ok({
                UnsignSet {
                    x: set.x.map(|v| v as u32),
                    y: set.y.map(|v| v as u32),
                    z: set.z.map(|v| v as u32),
                }
            })
        }
    })
    .parse(input)
}
pub fn parse_cmd(input: &str) -> IResult<&str, Cmd> {
    alt((
        preceded(tag_no_case("goto"), parse_set).map(Cmd::Goto),
        preceded(tag_no_case("move"), parse_set).map(Cmd::Move),
        preceded(tag_no_case("speed max"), parse_set_unsigned).map(Cmd::SpeedMax),
        preceded(tag_no_case("speed min"), parse_set_unsigned).map(Cmd::SpeedMin),
        preceded(tag_no_case("speed acc"), parse_set_unsigned).map(Cmd::SpeedAccel),
        preceded(tag_no_case("step_per_mm"), parse_set_unsigned).map(Cmd::StepPerMM),
        preceded(tag_no_case("add pos"), parse_3).map(Cmd::AddPos),
        preceded(tag_no_case("del pos"), parse_u32).map(Cmd::DelPos),
        value(Cmd::ListPos, tag_no_case("list pos")),
        value(Cmd::PumpOn, tag_no_case("pump on")),
        value(Cmd::PumpOff, tag_no_case("pump off")),
        value(Cmd::Start, tag_no_case("start")),
        value(Cmd::Stop, tag_no_case("stop")),
        value(Cmd::Home, tag_no_case("home")),
        value(Cmd::Help, tag_no_case("help")),
    ))
    .parse(input)
}
