use crate::minion;
use crate::common;
use crate::common_logic;
use crate::minion::EMINION;

#[derive(Clone, Copy)]
pub struct  EnemyUnitInfo
{
    pub minion_type : EMINION::KIND,
    pub spawn_tick : i32,
    pub spawn_num : i32
}

impl EnemyUnitInfo
{
    pub fn new(_mType : EMINION::KIND, _tick : i32, _num : i32) -> Self {
        return EnemyUnitInfo { minion_type: _mType, 
            spawn_tick: _tick, 
            spawn_num: _num }
    }
}

#[derive(Clone, Copy)]
struct EnemySpawnState
{
    elapsed : i32,
    spawned : i32
}

#[derive(Clone)]
pub struct EnemyGroup
{
    entries : Vec<(EnemyUnitInfo, EnemySpawnState)>
}

impl EnemyGroup
{
    pub fn New() -> Self {
        return EnemyGroup { entries : Vec::new() }
    }

    pub fn AddUnitInfo(&mut self, info : EnemyUnitInfo) {
        self.entries.push((info, EnemySpawnState { elapsed : 0, spawned : 0 }));
    }

    // world를 모르는 순수 스케줄링: 이번 틱에 스폰해야 할 종류만 돌려주고,
    // 실제 스폰(world.SpawnMinion)은 호출부에서 처리한다.
    pub fn Tick(&mut self) -> Vec<EMINION::KIND> {
        let mut spawn_list = Vec::new();

        for (info, state) in self.entries.iter_mut() {
            if state.spawned >= info.spawn_num {
                continue;
            }

            state.elapsed += 1;
            if state.elapsed >= info.spawn_tick {
                state.elapsed = 0;
                state.spawned += 1;
                spawn_list.push(info.minion_type);
            }
        }

        spawn_list
    }
}