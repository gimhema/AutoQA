

pub struct Geometry
{
    pub x : i32,
    pub y : i32   
}

pub struct CommonStatus
{
    pub health : i32,
    pub name : String
}

pub struct Actor
{
    pub status : CommonStatus,
    pub geometry : Geometry
}

impl CommonStatus
{
    fn Init(&self) {

    }
}

impl Geometry
{
    fn Init(&self) {

    }
}

impl Actor
{
    fn Init(&self) {

    }
}