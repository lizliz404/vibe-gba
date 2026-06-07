# Initial Prompt

```text
从头开发一个 rust 的 GBA 模拟器，支持到能正常渲染出 Pokemon Emerald.gba 这个游戏的画面，并能一路操作进入 onboard 完成后能在新手小镇中四处走动的状态。可以参考代码但不允许直接复用现成的 GBA 模拟引擎
```

## Publication Prompt

```text
把这个项目改名为 vibe-gba 开源到 github.com/doodlewind 下面，配套 README 写好，初始 prompt 写好，最后效果截图带上一两张即可
```

## Emerald Native-Loop Goal Prompt

```text
将 vibe-gba 从当前 milestone HLE 原型推进到 “Emerald 原生主循环可玩” 阶段：

目标：
在不直接操纵 Emerald 的玩家坐标、sprite/OAM、地图变量、剧情 flag、party 数据的前提下，只通过模拟 GBA 硬件和 KEYINPUT，让 Pokemon Emerald 从 fresh ROM 启动后，能够原生完成 New Game onboarding，进入搬家卡车，走出卡车，到达 Littleroot Town，并由游戏自己的 overworld / object event / script / movement 逻辑驱动玩家移动、碰撞、转向、动画和地图转场。

硬性约束：
1. 禁止使用 direct movement HLE。
2. 禁止在移动流程中直接写 player object 坐标、facing、moveDir、sprite frame、OAM 坐标。
3. 禁止用 HLE 直接推进 Route101/Birch/starter vars、flags、party 数据。
4. 允许继续使用调试 dump、save-state、screenshot、trace。
5. 允许实现或修复真实 GBA 硬件能力：CPU、BIOS/SWI、DMA、timer、IRQ、PPU、window、OBJ、save memory、waitstate、KEYINPUT。
6. Emerald-specific 代码只能作为诊断/断言使用，不能作为 gameplay 行为来源。

第一阶段验收：
1. 从 fresh ROM + fresh save 启动。
2. 脚本只输入 Start/A/方向键，不加载 game-derived save state。
3. 能看到 `NEW GAME / OPTION`、`BOY / GIRL`、名字输入、搬家卡车。
4. 在卡车内按方向键时，玩家移动由 Emerald 自己的 object/movement engine 产生，而不是模拟器直接改坐标。
5. 走出卡车进入 Littleroot 的地图转场由游戏原生脚本/事件触发。
6. 在 Littleroot 内上下左右移动时：
   - player object 坐标变化来自游戏代码；
   - facing/moveDir 正确；
   - OAM/sprite 动画由游戏自己的 sprite 更新路径产生；
   - 碰撞不能穿墙、不能穿房子、不能越界。

第二阶段验收：
1. 从 Littleroot 走到 Route 101 的北出口，转场由原生 map connection / event 触发。
2. Route 101 场景、NPC、Birch/Zigzagoon/bag 等 object 正常出现。
3. 与 object 交互时，A 键由游戏脚本系统消费，不由模拟器 HLE 消费。

最终可玩验收：
1. 从 fresh ROM 开始，不使用 save-state，不使用 gameplay HLE。
2. 人工可一路完成 onboarding。
3. 可在 Littleroot 和 Route 101 自由移动，方向、动画、碰撞、转场都由游戏原生逻辑驱动。
4. 能原生触发 Birch 救援、starter 选择、获得初始宝可梦。
5. debug dump 中不能出现 direct movement HLE / starter HLE 对 Emerald vars、flags、party 的直接写入。
6. README 提供一条可复现的 headless input-script 验收命令，以及人工验收步骤。

完成标准：
这个 Goal 只有在删除或禁用当前 direct movement HLE 和 starter/Route101 HLE 后，仍然能通过上述 fresh-ROM 人工验收和脚本验收，才算完成。
```
