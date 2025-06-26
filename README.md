# Minecraft Circuit JSON 语法说明

通过编译器，将项目json转换为schematic文件。


范例项目文件
```json

{
    "name":"template Circuit",
    "size":{"x":10,"y":10,"z":10},
    "imports":[
        {
            "modelName":"and",
            "modelType":"component",
            "path":"./components/and.json"
        },
        ...
    ],

    "components":[
        {
            "name":"and001",
            "model":"and",
            "position":{
                "x":0,
                "y":1,
                "z":2
            }
        }
    ],
    "wires":[
        {
            "name":"wire001",
            "start":{
                "x":1,
                "y":1,
                "z":2
            
            },
            "end":{
                "x":2,
                "y":1,
                "z":2
            },
            "baseMaterial":"stone"
        }
    ],
    "blocks":[
        {
            "position":{"x":0,"y":0,"z":0},
            "id":"repeator"
        }
    ],
    "inputs":[
        {"x":0,"y":0,"z":0},
        ...
    ],
    "outputs":[
        {"x":1,"y":0,"z":0},
        ...
    ]
}

```
## 项目文件结构

### imports

#### modelName

元件名称，如上面的and，将会被后面使用

#### type

元件类型，有以下几种：
- component：普通的组件，可以被放置在世界中，是直接对应一个nbt文件的
- circuit: 电路，对应一个项目json，依赖于其他的circuit和component

#### path

元件的路径，可以是相对路径，也可以是绝对路径，如果是相对路径，则以项目json为根目录。

### circuit

#### components

#### wires

##### baseMaterial

电线基底方块，填写方块id，如"stone"。

#### blocks

一些需要单独放置的方块。

### inputs

输入端口，可以有多个。

### outputs

输出端口，可以有多个。

上面inputs和outputs标签在项目被其他电路引用的时候是必须的，作为单独项目文件的时候是可选的。

## 元件json格式
元件json，如上面的and.json，格式如下
```json
{
    "name":"and",
    "type":"component",
    "nbt":"and.nbt",
    "size":[2,2,2],
    "inputs":[
        {"x":0,"y":0,"z":0},
        ...
    ],
    "outputs":[
        {"x":1,"y":0,"z":0},
        ...
    ]
}
```
注：这里的nbt可以是nbt, litematic和schematic文件。

## 编译

```bash
cargo build
```

## 使用方法

```bash
./mc_circuit_script <project_json_file> <output_schematic_file> [-h|-v]
```

