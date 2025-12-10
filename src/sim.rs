use std::{cmp::max, collections::HashMap, iter::Map, ops::Index};

use mc_schem::{block, Block, Schematic};

use crate::{
    check::{GlobalDirection, Graph},
    ModelObject, Port, Position,
};

///记录一个方块是否从某个方向计算过红石能量
struct CalcRecord {
    north: bool,
    south: bool,
    west: bool,
    east: bool,
    up: bool,
    down: bool,
}
impl CalcRecord {
    pub fn new() -> Self {
        Self {
            north: false,
            south: false,
            west: false,
            east: false,
            up: false,
            down: false,
        }
    }
    pub fn get_direct_result(&self, direction: GlobalDirection) -> bool {
        match direction {
            GlobalDirection::North => self.north,
            GlobalDirection::South => self.south,
            GlobalDirection::West => self.west,
            GlobalDirection::East => self.east,
            GlobalDirection::Up => self.up,
            GlobalDirection::Down => self.down,
        }
    }
    pub fn set_direct_result(&mut self, direction: GlobalDirection, result: bool) {
        match direction {
            GlobalDirection::North => self.north = result,
            GlobalDirection::South => self.south = result,
            GlobalDirection::West => self.west = result,
            GlobalDirection::East => self.east = result,
            GlobalDirection::Up => self.up = result,
            GlobalDirection::Down => self.down = result,
        }
    }
}
///
/// 对一个元件进行仿真，根据输入的红石能量生成输出红石能量表
pub fn simulate_component(
    model_object: &dyn ModelObject,
    inputs: &HashMap<String, i32>,
    lib_path: &str,
) -> HashMap<String, i32> {
    let nbt = lib_path.to_string().clone() + "/" + model_object.get_nbt_path().unwrap();
    let (mut nbt_obj, raw_meta) =
        Schematic::from_file(&nbt).unwrap_or_else(|x| panic!("failed to load nbt file {}", nbt));
    //合并成一个region
    nbt_obj.merge_regions(&Block::air());
    //准备输出表
    let mut outputs: HashMap<Port, i32> = HashMap::new();
    for port in model_object.get_outputs() {
        outputs.insert(port.clone(), 0);
    }
    //准备红石能量记录表
    let mut power_records: HashMap<Position, i32> = HashMap::new();
    let mut visited: HashMap<Position, CalcRecord> = HashMap::new();
    //初始化输入表
    for (p, power) in inputs.iter() {
        power_records.insert(
            model_object
                .get_inputs()
                .iter()
                .find(|x| x.name == *p)
                .unwrap()
                .position
                .clone(),
            *power,
        );
    }
    //开始仿真，BSF
    let mut to_visit: Vec<Position> = inputs
        .keys()
        .map(|x| {
            model_object
                .get_inputs()
                .iter()
                .find(|y| y.name == *x)
                .unwrap()
                .position
        })
        .collect();
    while let Some(pos) = to_visit.pop() {
        println!("visiting {}", pos);
        for neigh in pos.neighbors() {
            if let Some(blk) = nbt_obj.first_block_at(neigh.to_slice()) {
                println!("visiting neighbor {}:{}", neigh, blk.id);
            }
            /* 计算红石能量有几点：
            1. 能传播能量
            2. 有方块
            3. 有的点会需要重复计算取最高能量，为了防止BFS陷入死循环，需要记录已经计算过的点的哪些方向
            被计算过了
            */
            let relative_direct = GlobalDirection::direct(pos, neigh);
            if let Some(block) = nbt_obj.first_block_at(neigh.to_slice())
                && redstone_propagatable(block, relative_direct.clone())
                && (!visited.contains_key(&neigh)
                    || !visited[&neigh].get_direct_result(relative_direct))
            {
                //衰减后的能量
                let decayed_power = max(power_records[&pos] - 1, 0);
                //传播红石能量
                if block.id.contains("repeater")
                    || block.id.contains("torch")
                    || block.id.contains("redstone_block")
                {
                    //中继器
                    power_records.insert(neigh, 15);
                } else {
                    //计算能量，如果已有则取高
                    power_records
                        .entry(neigh)
                        .and_modify(|p| *p = max(*p, decayed_power))
                        .or_insert(decayed_power);
                }
                println!("calculatin power at {}:{}", neigh, power_records[&neigh]);
                //修改对应方向为计算过了
                visited
                    .entry(neigh)
                    .and_modify(|r| r.set_direct_result(relative_direct, true))
                    .or_insert({
                        let mut rec = CalcRecord::new();
                        rec.set_direct_result(relative_direct, true);
                        rec
                    });
                //能量衰减为零还是要继续传播的，这样才能计算出为零的输出口
                to_visit.push(neigh);
            }
        }
    }
    //设置输出表
    outputs.iter_mut().for_each(|(p, power)| {
        if let Some(pow) = power_records.get(&p.position) {
            *power = *pow;
        } else {
            println!("warning: output {} is not connected to any input", p.name);
            *power = 0;
        }
    });
    outputs
        .iter()
        .map(|(p, power)| (p.name.clone(), *power))
        .collect()
}
///检查红石能量能否传播到这个方块
/// power_source: 能源方向，从能源指向这个方块
fn redstone_propagatable(block: &Block, power_source: GlobalDirection) -> bool {
    block.id.contains("redstone_wire")
        || block.id.contains("redstone_torch")
        || block.id.contains("redstone_wall_torch")
        || block.id.contains("redstone_lamp")
        || block.id.contains("redstone_block")
        || !(block.id.contains("glass"))
        || (block.id.contains("repeater") && {
            //检查输入是不是从中继器的输入口输入
            let facing = GlobalDirection::from(block.attributes.get("facing").unwrap().as_str());
            //e.g. 中继器朝北，输入是从南向北输入，则不能传播
            facing == power_source
        })
}

/// 红石模拟结果
struct TruthTable {
    //列标题
    header: Vec<String>,
    //数据
    rows: Vec<Vec<i32>>,
}
impl TruthTable {
    fn new(header: Vec<String>) -> Self {
        Self {
            header,
            rows: Vec::new(),
        }
    }
    ///设置一个情况(一行)
    fn set(&mut self, inputs: Vec<i32>, outputs: Vec<i32>) {
        let mut row = Vec::new();
        inputs.iter().for_each(|x| row.push(*x));
        outputs.iter().for_each(|x| row.push(*x));
        //去掉已有的情况
        for (i, ro) in &mut self.rows.iter().enumerate() {
            if ro == &row {
                self.rows.remove(i);
                break;
            }
        }
        self.rows.push(row);
    }
    fn get(&self, inputs: Vec<i32>) -> Option<Vec<i32>> {
        for row in &self.rows {
            if row[..inputs.len()] == inputs {
                return Some(row[inputs.len()..].to_vec());
            }
        }
        None
    }
}
///模拟一个图，生成一个真值表
fn simulate_graph(graph: Graph) {
    let mut inp = vec![0; graph.inputs.len()];
    loop {
        //二进制方式加一
        todo!("一个输入情况的模拟，使用testch.lithematic_graph.json里面的图");
        for i in 0..inp.len() {
            inp[i] += 1;
            if inp[i] == 2 {
                inp[i] = 0;
                if i < inp.len() - 1 {
                    inp[i + 1] += 1;
                }
            }
        }
        if inp.iter().all(|x| *x == 1) {
            break;
        }
    }
}
