use std::{
    cmp::max,
    collections::HashMap,
    fs::OpenOptions,
    io::BufReader,
    iter::Map,
    ops::{Add, Index},
};

use mc_schem::{Block, Schematic, block};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    Circuit, ComponentModelObject, ModelObject, Port, Position, Wire,
    check::{GlobalDirection, Graph},
    error_begin,
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
/*
对于SimPoint用法的说明：
SimPoint是对元件和导线的仿真包装，也就是一个元件/导线一个SimPoint。
元件和导线对红石信号的影响被包装为SimFuncs，仿真时通过应用SimFuncs来计算信号。
Connections则表示SimPoint之间的连接关系。
*/
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SimFuncs {
    COPY,
    ///导线行为，将红石信号衰减个值之后输出
    WIRE,
    AND,
    OR,
    NOT,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
///仿真节点，导线的两端，元件的端口均使用此结构。
struct CalculationUnit {
    name: String,
    position: Position,
    //信号通过该节点时所做的运算
    func: SimFuncs,
    vars: Vec<u64>,
}
impl CalculationUnit {
    pub fn new(name: &str, position: Position, func: SimFuncs) -> Self {
        Self {
            name: name.to_string(),
            position,
            func,
            vars: Vec::new(),
        }
    }
    pub fn set_vars(&mut self, vars: Vec<u64>) {
        self.vars = vars;
    }
    pub fn get_func_from_model(model: &ComponentModelObject) -> SimFuncs {
        //暂时先写成硬编码，之后要改成可拓展形式
        match model.name.as_str() {
            "and" => SimFuncs::AND,
            "or" => SimFuncs::OR,
            "not" => SimFuncs::NOT,
            _ => SimFuncs::COPY,
        }
    }
}
/*
解释PointType的用法：
寻找连接时，对于导线的两端和元件的端口，标记为ENDING类型的点，
对于导线两端之外的线上其他部分上的点，标记为PART_OF_WIRE类型的点。

连接：两点处于同一位置。

对于ENDING类型的点，两种类型的点都能连接。
对于PART_OF_WIRE类型的点，只能和ENDING类型的点连接，不能和其他PART_OF_WIRE类型的点连接。
这样做的目的是防止导线中间的点和其他导线中间的点连接，导致错误的连接关系产生。
*/
#[derive(Clone, Copy, PartialEq, Eq)]
///标记PhysicalPoint的类型
enum PointType {
    ///导线的两端，元件的端口都属于这一类。
    ENDING,
    ///导线两端之外的其他部分上的点。
    PART_OF_WIRE,
}

#[derive(Clone)]
struct PhysicalPoint {
    name: String,
    simpoint: usize,
    position: Position,
    point_type: PointType,
}
impl PhysicalPoint {
    pub fn new(name: &str, simpoint: usize, position: Position, point_type: PointType) -> Self {
        Self {
            name: name.to_string(),
            simpoint,
            position,
            point_type,
        }
    }
}
#[derive(Debug, Clone)]
struct Connection {
    from: CalculationUnit,
    to: CalculationUnit,
}
impl Connection {
    pub fn new(from: CalculationUnit, to: CalculationUnit) -> Self {
        Self { from, to }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PowerPointType {
    INPUT,
    OUTPUT,
}
///红石信号点，存储着一个点上的红石信号，通常代表着元件端口和导线的两端
#[derive(Debug, Clone)]
struct PowerPoint {
    name: String,
    simpoint_index: usize,
    power: i32,
    powerpoint_type: PowerPointType,
}
impl PowerPoint {
    pub fn new(
        name: &str,
        simpoint_index: usize,
        power: i32,
        powerpoint_type: PowerPointType,
    ) -> Self {
        Self {
            name: name.to_string(),
            simpoint_index,
            power,
            powerpoint_type,
        }
    }
}
#[derive(Debug, Clone)]
///一场仿真，包含仿真所有的信息。
struct Simulation {
    units: Vec<CalculationUnit>,
    connections: Vec<Connection>,
    powerpoints: Vec<PowerPoint>,
}
impl Simulation {
    pub fn new() -> Self {
        Self {
            units: Vec::new(),
            connections: Vec::new(),
            powerpoints: Vec::new(),
        }
    }
}

///根据项目json文件，生成连接图。
fn generate_simulation_info(project: &Circuit) -> Simulation {
    //TODO 先生成 Vec<Connection>
    let mut simpoints = Vec::<CalculationUnit>::new();
    let mut pps = Vec::<PhysicalPoint>::new();
    let mut cons = Vec::<Connection>::new();
    let mut powerpoints = Vec::<PowerPoint>::new();
    //首先，给所有的导线、元件端口都赋予一个点
    for wire in project.wires.iter() {
        let mut wiresimp = CalculationUnit::new(&wire.name, wire.start.clone(), SimFuncs::WIRE);
        //TODO 导线的功能计算是需要设置导线长度的，这里还需要这样的代码
        wiresimp.set_vars(vec![calc_wire_effective_length(wire, &project)]);
        //从头至尾添加PhysicalPoint
        let pos_start = if wire.start.x == wire.end.x {
            wire.start.z
        } else {
            wire.start.x
        };
        let pos_end = if wire.end.x == wire.end.x {
            wire.end.z
        } else {
            wire.end.x
        };
        for j in (pos_start + 1)..pos_end {
            pps.push(PhysicalPoint::new(
                (wiresimp.name.clone() + j.to_string().as_str()).as_str(),
                simpoints.len(),
                wire.start.clone().add(Position {
                    x: if wire.start.x == wire.end.x {
                        wire.start.x
                    } else {
                        j
                    },
                    y: 0,
                    z: if wire.start.x == wire.end.x {
                        j
                    } else {
                        wire.start.z
                    },
                }),
                //首尾两点特殊类型
                match j {
                    pos_start => PointType::ENDING,
                    pos_end => PointType::ENDING,
                    _ => PointType::PART_OF_WIRE,
                },
            ));
            //首尾添加PowerPoint
            if j == pos_start || j == pos_end {
                powerpoints.push(PowerPoint::new(
                    (wiresimp.name.clone() + ".start").as_str(),
                    simpoints.len(),
                    0,
                    PowerPointType::INPUT,
                ));
                powerpoints.push(PowerPoint::new(
                    (wiresimp.name.clone() + ".end").as_str(),
                    simpoints.len(),
                    0,
                    PowerPointType::OUTPUT,
                ));
            }
        }
        simpoints.push(wiresimp);
    }
    for comp in project.components.iter() {
        //每一个元件就是一个点
        let model_obj = project.imports.iter().find(|&x| x.modelName == comp.model);
        if model_obj.is_none() {
            error_begin();
            panic!("model {} not found for component {}", comp.model, comp.name);
        }
        let model_obj = model_obj.unwrap();

        let realmodel: ComponentModelObject = {
            let f = OpenOptions::new().read(true).open(&model_obj.path);
            if f.is_err() {
                error_begin();
                panic!("failed to open model file {}", model_obj.path);
            }
            let f = f.unwrap();
            let reader = BufReader::new(f);
            serde_json::from_reader(reader).unwrap_or_else(|x| {
                error_begin();
                panic!("failed to parse model file {}: {}", model_obj.path, x);
            })
        };
        //
        let comppoint = CalculationUnit::new(
            &comp.name,
            comp.position.clone(),
            CalculationUnit::get_func_from_model(&realmodel),
        );
        simpoints.push(comppoint);
        //因为之后的判断线和元件连接是通过位置判断的，所以这里需要把元件的输入输出端口位置计算出来
        /*
         * 这里需要的做法是：
         * 一套SimPoint列表，导线两头各算一个，元件自身算一个。
         * 一套记录了位置的Point列表，每个Point记录了对应的SimPoint。
         * 然后根据Point连接关系，生成SimPoint的连接关系。
         */
        for port in realmodel.inputs.iter() {
            let pos = comp.position + port.position;
            pps.push(PhysicalPoint::new(
                &(comp.name.clone() + "." + &port.name),
                simpoints.len() - 1,
                pos,
                PointType::ENDING,
            ));
            //输入端口作为PowerPoint
            powerpoints.push(PowerPoint::new(
                &(comp.name.clone() + "." + &port.name),
                simpoints.len() - 1,
                0,
                PowerPointType::INPUT,
            ));
        }
        //add to ports
        for port in realmodel.outputs.iter() {
            let pos = comp.position + port.position;
            pps.push(PhysicalPoint::new(
                &(comp.name.clone() + "." + &port.name),
                simpoints.len() - 1,
                pos,
                PointType::ENDING,
            ));
            //输出端口作为PowerPoint
            powerpoints.push(PowerPoint::new(
                &(comp.name.clone() + "." + &port.name),
                simpoints.len() - 1,
                0,
                PowerPointType::OUTPUT,
            ));
        }
    }
    print!("SimPoints generated:{}\n", simpoints.len());
    print!("PhysicalPoints generated:{}\n", pps.len());
    //现在已经具备了记录物理地址的点pps，接下来开始根据pps建立联系。
    /*
    思路：
    1. 取出一个点A
    2. 遍历其他点，寻找位置相同的点B
    3. 建立连接，然后把B点从列表中移除，防止重复连接
    4. 所有B点处理完毕后，把A点从列表中移除
    5. 重复1-4，直到列表为空
     */
    while pps.len() > 0 {
        let a = pps.remove(0);
        //寻找位置相同的点,去除
        let _ = pps.iter().filter(|&point| {
            if point.position == a.position//位置相同
            //不能都是PART_OF_WIRE类型
                && !(a.point_type == PointType::PART_OF_WIRE
                    && point.point_type == PointType::PART_OF_WIRE)
            {
                //建立连接
                cons.push(Connection::new(
                    simpoints[a.simpoint].clone(),
                    simpoints[point.simpoint].clone(),
                ));
                return true;
            }
            false
        });
    }

    print!("Connections generated:{}\n", cons.len());
    //完成
    Simulation {
        units: simpoints,
        connections: cons,
        powerpoints: powerpoints,
    }
}

#[derive(Debug, Clone, Deserialize)]
///仿真输入/输出
struct SimulationPowerAssign {
    assignments: serde_json::Map<String, Value>,
}

fn simulate(simulation: &mut Simulation, inputs: SimulationPowerAssign) -> SimulationPowerAssign {
    //TODO 根据连接图和输入，进行仿真
    /*
    思路：
    根据输入信号，先给部分powerpoint赋值，然后根据连接图进行传播计算，直到所有的powerpoint都被计算过。
    传播计算时，根据CalculationUnit的func属性，进行不同的计算。
     */
    for (path, power) in inputs.assignments.iter() {
        println!("Input {}: {}", path, power);
        /*
        解析path的方法：
        就像访问结构体变量一样: component.port 或 wire.start/end
        先根据点名找到对应的PowerPoint，然后根据PowerPoint的simpoint_index找到对应的CalculationUnit。
         */
        let names = path.split('.').collect::<Vec<&str>>();
        let pp_opt = simulation
            .powerpoints
            .iter_mut()
            .find(|pp| pp.name == names[1] && simulation.units[pp.simpoint_index].name == names[0]);
        if pp_opt.is_none() {
            error_begin();
            panic!(
                "PowerPoint {} not found in simulation but assigned power",
                path
            );
        }
        let pp = pp_opt.unwrap();
        pp.power = power.as_i64().unwrap_or(0) as i32;
    }
    //初值设定完毕，开始传播计算

    unimplemented!()
}

pub fn simulate_circuit(circuit: &Circuit, inputs: SimulationPowerAssign) -> SimulationPowerAssign {
    let mut simulation = generate_simulation_info(circuit);
    simulate(&mut simulation, inputs)
}

///计算导线的有效长度，考虑中继器
fn calc_wire_effective_length(wire: &Wire, project: &Circuit) -> u64 {
    //TODO 计算导线的有效长度
    //目前先返回长度，不考虑中继器
    wire.end.distance(wire.start)
}

///根据连接集合生成连接图
fn generate_connention_map(
    con_set: Vec<Connection>,
) -> HashMap<CalculationUnit, Vec<CalculationUnit>> {
    let mut simpoints = Vec::<CalculationUnit>::new();
    //先收集所有的SimPoint
    con_set.iter().for_each(|c| {
        if !simpoints.contains(&c.from) {
            simpoints.push(c.from.clone());
        }
        if !simpoints.contains(&c.to) {
            simpoints.push(c.to.clone());
        }
    });
    //现在连接建立完毕，开始生成连接图
    let mut conmap: HashMap<CalculationUnit, Vec<CalculationUnit>> = HashMap::from(
        //先生成空表
        simpoints
            .iter()
            .map(|simp| (simp.clone(), Vec::new()))
            .collect::<HashMap<CalculationUnit, Vec<CalculationUnit>>>(),
    );
    for sim in simpoints.iter() {
        con_set.iter().for_each(|c| {
            if c.from.name == sim.name || c.to.name == sim.name {
                conmap
                    .get_mut(&sim)
                    .expect("FATAL: conmap does not have the according SimPoint after generating")
                    .push(if c.from.name == sim.name {
                        c.to.clone()
                    } else {
                        c.from.clone()
                    }); //把连接的另一端加入
            }
        });
    }
    conmap
}
