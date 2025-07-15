use crate::*;

// 定义数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Dot {
    pos: Position,
    distance: i32,   // 自起点的距离
    type_: NodeType, // 输入输出等
}
impl Display for Dot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "({},{},{},{})",
            self.pos.x, self.pos.y, self.pos.z, self.type_
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Edge {
    start: usize,
    end: usize,
    length: i32,        // 导线长度
    direct: EdgeDirect, // 单向还是双向
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum EdgeDirect {
    Bidirectional,
    Reversed,
    Nonreversed,
}
impl Display for EdgeDirect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                EdgeDirect::Bidirectional => "bidirectional",
                EdgeDirect::Reversed => "reversed",
                EdgeDirect::Nonreversed => "nonreversed",
            }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum NodeType {
    Input,
    Output,
}
impl Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                NodeType::Input => "input",
                NodeType::Output => "output",
            }
        )
    }
}

// 辅助结构体和函数
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum RepeaterDirection {
    Forward,
    Backward,
}
///全局方向
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum GlobalDirection {
    South,
    North,
    West,
    East,
}
impl From<&str> for GlobalDirection {
    fn from(value: &str) -> Self {
        match value {
            "South" | "south" => GlobalDirection::South,
            "North" | "north" => GlobalDirection::North,
            "West" | "west" => GlobalDirection::West,
            "East" | "east" => GlobalDirection::East,
            &_ => panic!("Invalid direction"),
        }
    }
}
#[derive(Serialize, Deserialize)]
pub struct Graph {
    dots: Vec<Dot>,
    edges: Vec<Edge>,
    outputs: Vec<Dot>,
    inputs: Vec<Dot>,
}

pub fn check_circuit(obj: &Circuit, model_objects: &Vec<Box<dyn ModelObject>>) -> bool {
    // 构建图
    let mut isok = true;

    let graph = create_graph(obj, model_objects);
    if graph.is_none() {
        error_begin();
        println!("failed to construct graph from circuit");
        return false;
    }
    let graph = graph.unwrap();

    #[cfg(debug_assertions)]
    println!("dots:");
    for dot in &graph.dots {
        println!("{},", dot);
    }
    println!("edges:");
    for edge in &graph.edges {
        println!(
            "({},{},{},{})",
            edge.start, edge.end, edge.length, edge.direct
        );
    }
    println!("blocks:");
    for block in &obj.blocks {
        println!(
            "{}:({},{},{}),{}",
            block.id,
            block.position.x,
            block.position.y,
            block.position.z,
            {
                if let Some(properties) = block.properties.as_ref() {
                    properties.clone()
                } else {
                    Properties::default()
                }
            }
        );
    }

    // 检查可达性
    for output in &graph.outputs {
        let output_idx = graph.find_dot(output.pos.clone()).unwrap();
        let reachables = graph.get_reachables(output_idx);
        for &reachable_idx in &reachables {
            let reachable = &graph.dots[reachable_idx];
            let distance = graph.get_distance(output_idx, reachable_idx);

            if cfg!(debug_assertions) {
                println!(
                    "checking reachability from {} to {}, distance: {}",
                    output.pos, graph.dots[reachable_idx].pos, distance
                );
            }

            if distance > MAX_REDSTONE_DISTANCE {
                #[cfg(debug_assertions)]
                println!("found long path, checking if it has repeater");

                let paths = graph.get_paths(output_idx, reachable_idx);
                let mut reachable_with_repeater = false;
                for path in paths {
                    let mut energy = MAX_REDSTONE_DISTANCE;
                    let mut last_dot = output_idx;
                    for &dot_idx in &path {
                        let dot = &graph.dots[dot_idx];
                        if let Some(edge) = graph.find_edge(last_dot, dot_idx) {
                            if edge.direct == EdgeDirect::Reversed {
                                energy = MAX_REDSTONE_DISTANCE;
                            } else if edge.direct == EdgeDirect::Nonreversed {
                                if let Some(block) =
                                    obj.blocks.iter().find(|b| b.position == dot.pos)
                                {
                                    if block.id == "repeater" {
                                        energy = MAX_REDSTONE_DISTANCE;
                                    } else {
                                        energy -= edge.length;
                                        if energy <= 0 {
                                            reachable_with_repeater = false;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        last_dot = dot_idx;
                    }
                    if energy > 0 {
                        reachable_with_repeater = true;
                        break;
                    }
                }
                if !reachable_with_repeater {
                    error_begin();
                    println!(
                        "unreachable output due to running out of redstone power : {} -> {}",
                        output, reachable.pos
                    );
                    isok = false;
                }
            }
        }
    }

    isok
}

impl Graph {
    fn new() -> Self {
        Graph {
            edges: vec![],
            outputs: vec![],
            inputs: vec![],
            dots: vec![],
        }
    }

    fn add_dot(&mut self, dot: Dot) -> usize {
        let idx = self.dots.len();
        match dot.type_ {
            NodeType::Input => self.inputs.push(dot.clone()),
            NodeType::Output => self.outputs.push(dot.clone()),
            _ => {}
        }
        self.dots.push(dot);
        idx
    }

    fn find_dot(&self, pos: Position) -> Option<usize> {
        self.dots.iter().position(|d| d.pos == pos)
    }

    fn find_dot_or_add(&mut self, pos: Position) -> usize {
        if let Some(idx) = self.find_dot(pos.clone()) {
            idx
        } else {
            self.add_dot(Dot {
                pos,
                distance: i32::MAX,
                type_: NodeType::Input, // 默认为输入
            })
        }
    }

    fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    fn find_edge(&self, start: usize, end: usize) -> Option<&Edge> {
        self.edges.iter().find(|e| e.start == start && e.end == end)
    }
    fn get_dot(&self, pos: Position) -> &Dot {
        self.dots.iter().find(|d| d.pos == pos).unwrap()
    }
    fn get_distance(&self, start: usize, end: usize) -> i32 {
        //一个简单的Dijkstra
        let mut visits = vec![start];
        //(索引，距离)
        let mut distances = HashMap::<usize, i32>::new();
        let mut visited = vec![];
        distances.insert(start, 0);
        while visits.len() > 0 {
            let current = visits.remove(0);
            visited.push(current);
            if self.dots[current].pos == self.dots[end].pos {
                return distances[&end];
            }
            for wire in &self.edges {
                if wire.start == current || wire.end == current {
                    let other = if wire.start == current {
                        wire.end
                    } else {
                        wire.start
                    };
                    if !visited.contains(&other) {
                        let new_distance = distances[&current] + wire.length;
                        if let Some(old_distance) = distances.get(&other) {
                            if new_distance < *old_distance {
                                distances.insert(other, new_distance);
                                visits.push(other);
                            }
                        } else {
                            distances.insert(other, new_distance);
                            visits.push(other);
                        }
                    }
                }
            }
        }
        i32::MAX
    }

    fn get_neighbors(&self, dot: usize) -> Vec<usize> {
        self.edges
            .iter()
            .filter(|e| e.start == dot || e.end == dot)
            .map(|e| if e.start == dot { e.end } else { e.start })
            .collect()
    }

    fn get_reachables(&self, dot: usize) -> Vec<usize> {
        let mut visited = vec![false; self.dots.len()];
        let mut reachables = vec![];
        let mut queue = vec![dot];

        while let Some(current) = queue.pop() {
            if visited[current] {
                continue;
            }
            visited[current] = true;
            reachables.push(current);
            for &neighbor in &self.get_neighbors(current) {
                queue.push(neighbor);
            }
        }

        reachables
    }

    fn get_paths(&self, start: usize, end: usize) -> Vec<Vec<usize>> {
        let mut paths = vec![];
        let mut queue = vec![(start, vec![start])];

        while let Some((current, path)) = queue.pop() {
            if current == end {
                paths.push(path.clone());
            } else {
                for edge in &self.edges {
                    if edge.start == current {
                        let mut new_path = path.clone();
                        new_path.push(edge.end);
                        queue.push((edge.end, new_path));
                    }
                }
            }
        }

        paths
    }
}

fn wire_positions(start: &Position, end: &Position) -> Vec<Position> {
    // 假设导线是直线连接的，这里简单处理为从start到end的所有位置
    let mut positions = vec![];
    let (dx, dy, dz) = (end.x - start.x, end.y - start.y, end.z - start.z);
    let steps = dx.abs().max(dy.abs()).max(dz.abs());

    for i in 0..=steps {
        positions.push(Position {
            x: start.x + dx * i / steps,
            y: start.y + dy * i / steps,
            z: start.z + dz * i / steps,
        });
    }

    positions
}

fn wire_length(start: &Position, end: &Position) -> i32 {
    // 计算曼哈顿距离
    (start.x - end.x).abs() + (start.y - end.y).abs() + (start.z - end.z).abs()
}

fn repeater_direction(
    pos: &Position,
    blocks: &Vec<BlockInfo>,
    wire_direction: GlobalDirection,
) -> Option<RepeaterDirection> {
    //中继器的方向存储在项目json中
    for block in blocks {
        if block.position == *pos && block.id.contains("repeater") {
            let direct = GlobalDirection::from(
                block
                    .properties
                    .as_ref()
                    .expect("repeater block has no properties")
                    .facing
                    .as_str(),
            );
            if direct == wire_direction {
                return Some(RepeaterDirection::Forward);
            } else {
                return Some(RepeaterDirection::Backward);
            }
        }
    }

    None
}

const MAX_REDSTONE_DISTANCE: i32 = 15; // 红石的最大传播距离

///
/// ## 构建连接图，表示元件端口之间的连接关系。
///
/// circuit: 电路的元数据
///
/// model_objects: 模型对象列表
///
pub fn create_graph(circuit: &Circuit, model_objects: &Vec<Box<dyn ModelObject>>) -> Option<Graph> {
    // 构建图
    let mut graph = Graph::new();
    let mut isok = true;

    // 添加输入和输出节点
    for port in &circuit.inputs {
        graph.add_dot(Dot {
            pos: port.position,
            distance: i32::MAX,
            type_: NodeType::Output, //外部输入在内部视为输出
        });
    }
    //从ComponentModel中获取输入输出节点
    for comp in &circuit.components {
        let model = model_objects.iter().find(|m| m.get_name() == comp.model);
        if model.is_none() {
            error_begin();
            println!("Error: 模型 {} 不存在", comp.model);
            return None;
        }
        let model = model.unwrap();

        model.get_inputs().iter().for_each(|port| {
            graph.add_dot(Dot {
                pos: port.position + comp.position,
                distance: i32::MAX,
                type_: NodeType::Input,
            });
        });
        model.get_outputs().iter().for_each(|port| {
            graph.add_dot(Dot {
                pos: port.position + comp.position,
                distance: i32::MAX,
                type_: NodeType::Output,
            });
        });
    }

    // 添加导线节点和边
    for wire in &circuit.wires {
        let start_idx = graph.find_dot_or_add(wire.start.clone());
        let end_idx = graph.find_dot_or_add(wire.end.clone());
        let mut direct = EdgeDirect::Bidirectional;
        let mut conflict = false;
        // 检查导线上的中继器
        let positions = wire_positions(&wire.start, &wire.end);
        for pos in positions {
            if let Some(block) = circuit.blocks.iter().find(|b| b.position == pos) {
                if block.id.contains("repeater") {
                    let new_direct = match repeater_direction(&pos, &circuit.blocks, {
                        if wire.start.x < wire.end.x {
                            GlobalDirection::East
                        } else if wire.start.x > wire.end.x {
                            GlobalDirection::West
                        } else if wire.start.z < wire.end.z {
                            GlobalDirection::South
                        } else {
                            GlobalDirection::North
                        }
                    }) {
                        Some(RepeaterDirection::Forward) => EdgeDirect::Nonreversed,
                        Some(RepeaterDirection::Backward) => EdgeDirect::Reversed,
                        None => EdgeDirect::Bidirectional, // 默认双向
                    };
                    if direct == EdgeDirect::Nonreversed && new_direct == EdgeDirect::Reversed
                        || direct == EdgeDirect::Reversed && new_direct == EdgeDirect::Nonreversed
                    {
                        error_begin();
                        println!(
                            "a wire has two for more repeaters whose directions are opposite:{},starting from {} to {}, ignoring this wire",
                            wire.name, wire.start, wire.end
                        );
                        isok = false;
                        conflict = true;
                    } else {
                        direct = new_direct;
                    }
                }
            }
        }
        if !conflict {
            graph.add_edge(Edge {
                start: start_idx,
                end: end_idx,
                length: wire_length(&wire.start, &wire.end),
                direct,
            });
        }
        //线的首尾节点为导线上的节点
        if graph.dots.iter().find(|d| d.pos == wire.start).is_none() {
            graph.add_dot(Dot {
                pos: wire.start.clone(),
                distance: 0,
                type_: NodeType::Output,
            });
        }
        if graph.dots.iter().find(|d| d.pos == wire.end).is_some() {
            graph.add_dot(Dot {
                pos: wire.end.clone(),
                distance: 0,
                type_: NodeType::Output,
            });
        }
    }
    Some(graph)
}
