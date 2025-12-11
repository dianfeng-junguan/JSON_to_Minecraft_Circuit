mod check;
mod sim;
mod wiring;
use ansi_term::Color::{*};
use clap::Parser;
use flate2::Compression;
use std::{any::Any, fmt::Display, fs::{File, OpenOptions}, io::{BufReader, Read, Write}, ops::Add};
use serde_derive::{Deserialize, Serialize};
use mc_schem::{region::WorldSlice, schem::{LitematicaSaveOption, Schematic}, Block, Region};
use std::collections::{HashMap, HashSet};
use check::*;

use crate::sim::{do_simulation, simulate_component};
#[derive(Debug, Clone, Serialize, Deserialize,PartialEq, Eq,Hash,Copy)]
struct Position{
    x: i32,
    y: i32,
    z: i32
}
impl Position {
    pub fn to_slice(&self) -> [i32;3] {
        [self.x,self.y,self.z]
    }
    pub fn neighbors(&self) -> Vec<Position> {
        vec![*self+Position{x:1,y:0,z:0},*self+Position{x:-1,y:0,z:0},*self+Position{x:0,y:1,z:0},*self+Position{x:0,y:-1,z:0},*self+Position{x:0,y:0,z:1},*self+Position{x:0,y:0,z:-1}]
    }
    pub fn distance(&self,pos2: Position) -> u64 {
        ((self.x - pos2.x).abs() + (self.y - pos2.y).abs() + (self.z - pos2.z).abs()) as u64
    }
}
impl Add for Position {
    type Output=Position;

    fn add(self, rhs: Self) -> Self::Output {
        Position
        {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z
        }
    }
}
impl Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,"({},{},{})",self.x,self.y,self.z)
    }
}
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
struct ImportItem{
    modelName: String,
    modelType: String,
    path: String,
}
#[derive(Serialize, Deserialize,Clone)]
///## Component
/// 元件对象，存储在Circuit中
/// 
struct Component{
    name: String,
    model: String,
    position: Position,
}
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
struct Wire{
    name: String,
    start: Position,
    end: Position,
    baseMaterial: String,
}
#[derive(Serialize, Deserialize,Clone)]
struct Properties{
    facing: String,
    delay: i32,
    locked: bool,
    powered: bool,
    power: i32
}
impl Display for Properties {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,"{{\n facing:{},\n delay:{},\n locked:{},\n powered:{},\n power:{}}}",
            self.facing,self.delay,self.locked,self.powered,self.power)
    }
}
impl Default for Properties {
    fn default() -> Self {
        Self { facing: Default::default(), delay: Default::default(), locked: Default::default(), powered: Default::default(), power: Default::default() }
    }
}
#[derive(Serialize, Deserialize)]
struct BlockInfo{
    position: Position,
    id:String,
    properties:Option<Properties>
}
#[derive(Serialize, Deserialize,Clone,PartialEq,Eq,Hash,Debug)]
struct Port{
    name: String,
    position: Position,
}
#[derive(Serialize, Deserialize)]
///## Circuit
/// 项目文件的存储对象。
struct Circuit{
    name: String,
    size: Position,
    imports: Vec<ImportItem>,
    components: Vec<Component>,
    wires: Vec<Wire>,
    blocks:Vec<BlockInfo>,
    inputs:Vec<Port>,
    outputs:Vec<Port>
}
///## ModelObject
/// 项目文件中存储的元件和导线所具有的共性接口
trait ModelObject:Any {
    fn get_name(&self) -> &str;
    fn get_type(&self) -> &str;
    fn get_inputs(&self) -> &Vec<Port>;
    fn get_outputs(&self) -> &Vec<Port>;
    fn get_nbt_path(&self) -> Option<&str>;
    fn as_any(&self) -> &dyn Any;
}
#[derive(Serialize, Deserialize)]
///## ComponentModelObject
/// 元件导入模型对象
/// 
/// 包含元件的名称、类型、NBT、大小、输入、输出等信息
struct ComponentModelObject {
    name: String,
    modelType: String,
    nbt:String,
    size: [i32;3],
    inputs: Vec<Port>,
    outputs: Vec<Port>,
}
impl ModelObject for ComponentModelObject {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_inputs(&self) -> &Vec<Port> {
        &self.inputs
    }

    fn get_outputs(&self) -> &Vec<Port> {
        &self.outputs
    }
    
    fn get_type(&self) -> &str {
        &self.modelType
    }
    
    fn get_nbt_path(&self) -> std::option::Option<&str> {
        Some(self.nbt.as_str())
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl ModelObject for Circuit {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_inputs(&self) -> &Vec<Port> {
        &self.inputs
    }

    fn get_outputs(&self) -> &Vec<Port> {
        &self.outputs
    }
    
    fn get_type(&self) -> &str {
        "circuit"
    }
    
    fn get_nbt_path(&self) -> Option<&str> {
        None
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}
///## ModelObjectItem
/// 一个用来包装ModelObject的结构体，包含内部
/// ModelObject具体类型的信息。
struct ModelObjectItem{

}

#[derive(Parser,Debug)]
#[command(version("1.0.0"),about, long_about=None)]
struct CommandLineArgs{
    #[clap(short, long)]
    input_json: String,
    #[clap(short, long)]
    output_path: String,
    #[clap(short, long)]
    decomp_path: Option<String>,
    #[clap(short, long)]
    generate_component_json: bool,
    #[clap(short, long)]
    check_circuit:bool,//检查电路的连接，红石可达性等
    #[clap(short,long)]
    library:Option<String>,//导入的组件库
    #[clap(long)]
    graph_json:bool,//生成连接图的json文件
    #[clap(short, long)]
    simulate_input_path:Option<String>//模拟电路运行输入文件路径
}
fn main() {
    let args=CommandLineArgs::parse();
    let input_json=args.input_json;
    let output_path=args.output_path;
    let decomp_path=args.decomp_path.unwrap_or_default();
    let decomp_flag=false;
    //生成schematic的时候是否附带生成把它视为component的json文件
    let genereate_component_flag=args.generate_component_json;
    if decomp_flag {
        error_begin();
        unimplemented!("Decompiling not implemented yet");
    }
    
    //否则输入文件视为circuit文件
    //编译成schematic
    let mut jsonfile=OpenOptions::new()
    .read(true)
    .open(&input_json).expect("failed to open input json file");
    let mut json_content=Vec::<u8>::new();
    jsonfile.read_to_end(&mut json_content).unwrap();
    let json_content=String::from_utf8_lossy(&json_content);
    let obj:Circuit=serde_json::from_str(&json_content).unwrap_or_else(|x|{
        error_begin();
        panic!("failed to parse input json file {}:\n{}",&input_json,x.to_string());
    });
    //TODO 仿真需要输入两个文件: 电路json和输入json
    //仿真
    if let Some(simulate_input_path)=args.simulate_input_path {
        let input_component_model:ComponentModelObject=serde_json::from_reader(BufReader::new(File::open(input_json).expect("failed to open input json file"))).unwrap();
        let mut inputs=String::new();
        OpenOptions::new().read(true).open(simulate_input_path).expect("failed to open simulate input file").read_to_string(&mut inputs).expect("failed to read simulate input file");
        let output_json=do_simulation(&obj, inputs);
        let mut output_file=OpenOptions::new().write(true).create(true).truncate(true).open(output_path.clone()).unwrap_or_else(|e| {
            error_begin();
            panic!("failed to open output json file {}",output_path.clone());
        });
        output_file.write(output_json.as_bytes()).expect("failed to write output json file");
        println!("generated output json file {}",output_path);
        return;
    }
    //存放读取的元件和子电路json对象，缓存
    let mut model_objects:Vec<Box<dyn ModelObject>>=vec![];
    let mut schem:Schematic=Schematic::new();
    //解析导入，存入缓存方便后面取用
    for import_item in obj.imports.iter() {
        let path={
            if let Some(library_path)=args.library.clone() {
                library_path+"/"+import_item.path.as_str()
            }else {
                import_item.path.clone()
            }
        };
        let model_type=import_item.modelType.as_str();
        let rd=OpenOptions::new().read(true)
        .open(&path).expect(format!("failed to open import file {}",&path).as_str());
        match model_type {
            "component"=>{
                //元件类型
                let model_obj=Box::<ComponentModelObject>::new(serde_json::from_reader(rd).unwrap());
                model_objects.push(model_obj);
            },
            "circuit"=>{
                let model_obj=Box::<Circuit>::new(serde_json::from_reader(rd).unwrap());
                model_objects.push(model_obj);
            }
            _=>{
                error_begin();
                panic!("Unsupported model type: {}",model_type);
            }
        }
    }

    
    //检查电路
    if args.check_circuit && !check_circuit(&obj, &model_objects){
        error_begin();
        println!("Error: circuit check failed. Stop compiling.");
        return;
    }else if args.check_circuit {
        println!("check done. No problem found.")
    }
    if args.graph_json {
        let graphstr=serde_json::to_string(&create_graph(&obj, &model_objects)).unwrap();
        let mut graphjson=OpenOptions::new().write(true).create(true).truncate(true).open(output_path.clone()+"_graph.json").unwrap_or_else(|e| {
            error_begin();
            panic!("failed to open graph json file {}",output_path.clone()+"_graph.json");
        });
        graphjson.write(graphstr.as_bytes()).expect("failed to write graph json file");
        println!("generated graph json file {}_graph.json",output_path);
    }
    
    //下面开始编译
    //创建一个region
    let global_region=Region::with_shape(obj.size.to_slice());
    schem.regions.push(global_region);
    let global_region=&mut schem.regions[0];
    //解析元件
    for component in obj.components {
        println!("Component:{},Model:{},Position:({},{},{})",component.name,component.model,component.position.x,component.position.y,component.position.z);
        let model_name=component.model.as_str();
        //找到对应导入
        let model_import_item=model_objects.iter().find(|&x| {
            if x.get_name() == model_name {
                return true;
            }
            false
        }).unwrap_or_else(|| {
            error_begin();
            println!("Error: Model {} not found in imports",model_name);
            panic!("Model {} not found in imports",model_name);
        });
        //根据不同的model_type进行处理，然后放置到schematic的region中
        match model_import_item.get_type() {
            "component"=>{
                // 元件，寻找它的nbt
                let nbt_path={
                    let relative=model_import_item.get_nbt_path().unwrap();
                    if let Some(library_path) = args.library.as_ref()  {
                        library_path.to_owned()+"/"+relative
                    }else {
                        relative.to_string()
                    }
                };
                let (mut nbt_obj,raw_meta)=Schematic::from_file(&nbt_path).unwrap_or_else(|x|{panic!("failed to load nbt file {}",nbt_path)});
                //nbt合并到一个region防止多个region
                nbt_obj.merge_regions(&Block::air());
                //放置到schem的region
                let component_shape=nbt_obj.shape();
                //开始放置
                for x in 0..component_shape[0] {
                    for y in 0..component_shape[1] {
                        for z in 0..component_shape[2] {
                            if let Some(blk)=nbt_obj.regions[0].block_at([x,y,z]) {
                                global_region.set_block(
                                    [x+component.position.x,
                                    y+component.position.y,
                                    z+component.position.z], 
                                    blk).unwrap_or_else(|_|{
                                        println!("failed to place block {} at ({},{},{})",blk.id,x+component.position.x,y+component.position.y,z+component.position.z);
                                        let global_shape=global_region.shape();
                                        if  global_shape[0]<=x||global_shape[1]<=y||global_shape[2]<=z {
                                            panic!("block position out of range: ({},{},{}), the component size is ({},{},{})",
                                            x+component.position.x,y+component.position.y,z+component.position.z,
                                            component_shape[0],component_shape[1],component_shape[2]);
                                        }
                                    });
                            }
                        }
                    }
                }
            },
            "circuit"=>{
                error_begin();
                unimplemented!("Circuit import not implemented yet")
            },
            _=>{
                error_begin();
                panic!("Unsupported model type: {}",model_import_item.get_type());
            }
        }
    }
    //解析导线
    for wire in obj.wires {
        let base_block=Block::from_id(&wire.baseMaterial)
        .expect("err: invalid base material");
        let mut start_pos=wire.start.to_slice();
        let mut end_pos=wire.end.to_slice();
        fill_block(start_pos, end_pos, base_block, global_region);
        start_pos[1]+=1;
        end_pos[1]+=1;
        //放置导线
        fill_block(start_pos, end_pos, Block::from_id("redstone_wire").unwrap(), global_region);

    }
    //解析方块
    for block in obj.blocks {
        let block_id=block.id.as_str();
        let block_pos=block.position.to_slice();
        let block_block=Block::from_id(block_id)
        .expect(format!("err: invalid block id {}",block_id).as_str());
        global_region.set_block(block_pos,&block_block).unwrap();
    }
    //完毕，保存
    let save_option=LitematicaSaveOption{
        compress_level: Compression::default(),
        rename_duplicated_regions: true, 
    };
    //生成对应component的json文件
    if genereate_component_flag {
        let component_json=ComponentModelObject{
            name: obj.name.clone(),
            modelType: "component".to_string(),
            nbt: output_path.to_string(),
            size: obj.size.to_slice(),
            inputs: obj.inputs.clone().iter_mut().enumerate().map(|(i,p)| {p.name=format!("input{}",i);p.clone()}).collect(),
            outputs: obj.outputs.clone().iter_mut().enumerate().map(|(i,p)| {p.name=format!("output{}",i);p.clone()}).collect(),
        };
        let mut component_json=serde_json::to_string(&component_json).unwrap();
        let mut component_file=OpenOptions::new()
        .write(true)
        .create(true).truncate(true)
        .open(format!("{}.json",output_path)).unwrap();
        component_file.write_all(component_json.as_bytes()).unwrap();
        println!("Component json file saved to {}.json",output_path);
    }
    schem.save_litematica_file(&output_path, &save_option).expect("error: failed to save schematic file");
    println!("Schematic saved to {}",output_path);
}


fn fill_block(start:[i32;3],end:[i32;3],block:Block,region:&mut Region){
    for x in start[0]..=end[0] {
        for y in start[1]..=end[1] {
            for z in start[2]..=end[2] {
                region.set_block([x,y,z],&block).unwrap();
            }
        }
    }
}
fn error_begin(){
    print!("{}",Red.paint("error: "));
}
