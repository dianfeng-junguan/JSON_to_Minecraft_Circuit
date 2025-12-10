```rust
Circuit
- import:ImportItem
    - model:String //导入元件模型的名字
        对应 ComponentModelObject //(对应一个元件.json)
        
- components:Component
- wires:Wire
- blocks:BlockInfo
    porperties: Properties //特殊的方块属性
- inputs:Port
- outputs:Port

```