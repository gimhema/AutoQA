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
    pub speed : i32
}

pub struct Actor
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
    }
}

impl Geometry
{
    fn  Init(&mut self) {
        self.x = 0;
        self.y = 0;
    }
}

impl Actor
{
    fn Init(&mut self) {
        self.status.Init();
        self.geometry.Init();   
    }
}