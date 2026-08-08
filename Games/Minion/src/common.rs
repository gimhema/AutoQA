use crate::config;


pub struct Geometry
{
    pub x : i32,
    pub y : i32   
}

pub struct CommonStatus
{
    pub health : i32,
    pub name : String,
    pub speed : i32,
    pub power : i32,
    pub defense : i32
}

pub struct ActorInfo
{
    pub status : CommonStatus,
    pub geometry : Geometry
}

impl CommonStatus
{
    fn Init(&mut self) {
        self.health = 0;
        self.name = String::new();
        self.speed = 0;
        self.power = 0;
        self.defense = 0;
    }
}

impl Geometry
{
    pub fn  Init(&mut self) {
        self.x = 0;
        self.y = 0;
    }

    pub fn Update(&mut self, x : i32, y : i32) {
        self.x = x;
        self.y = y;
    }
}

impl ActorInfo
{
    pub fn Init(&mut self) {
        self.status.Init();
        self.geometry.Init();   
    }
}