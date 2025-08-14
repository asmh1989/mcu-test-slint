# MCU测试工具

这是一个使用Rust和Slint框架开发的MCU测试工具，具有现代化的图形用户界面。

## 界面美化特性 ✨

### 🎨 视觉设计
- **浅色主题**：使用现代浅色配色方案，告别黑色背景
- **圆角边框**：12px圆角设计，更加现代
- **阴影效果**：添加微妙阴影，增强层次感
- **明显区分**：每个功能区域都有清晰的边框和背景区分
- **响应式布局**：适应不同窗口大小

### 🎯 布局优化
- **左侧面板**：三个部分均匀占满整个高度
  - 连接面板：固定90px高度
  - 地址面板：弹性高度，最小140px
  - IO控制面板：弹性高度，最小140px
- **右侧面板**：优化的文件操作区域
  - 状态显示区：单行文字高度（40px）
  - 内容显示区：占据剩余空间

## 界面布局

### 左侧面板 (40%宽度)

#### 上部：连接控制面板 (`connection-panel.slint`)
- **连接MCU** 按钮：显示MCU连接状态
- **COM5** 按钮：显示当前选择的串口
- **已连接** 按钮：显示连接状态，点击可切换连接

#### 中部：地址操作面板 (`address-panel.slint`)
- **地址输入框**：输入要操作的寄存器地址
- **参数输入/显示框**：输入参数或显示读取的数据
- **读取地址** 按钮：从指定地址读取数据
- **写入地址** 按钮：向指定地址写入数据

#### 下部：IO控制面板 (`io-control-panel.slint`)
- **IO1控制**：置低按钮、置高按钮、IO1显示
- **IO2控制**：置低按钮、置高按钮、IO2显示

### 右侧面板 (60%宽度) - `file-operation-panel.slint`

#### 上部：文件状态显示 🆕
- **状态标签**：显示文件操作结果，支持颜色状态区分
  - 🟢 **绿色**：文件读取成功，显示完整文件路径
  - 🔴 **红色**：读取错误，显示错误原因
  - 🟡 **黄色**：操作进行中
  - 🔵 **蓝色**：配置操作完成
  - ⚪ **灰色**：默认状态

#### 下部：左右布局
- **左侧**：三个功能按钮垂直排列
  - **读取文件**：从文件中读取配置
  - **读取器件**：从设备读取当前配置  
  - **配置器件**：将配置写入设备

- **右侧**：内容显示区域
  - **寄存器地址与参数表**：显示文件内容或设备状态

## 状态管理系统 🔄

### 文件状态显示
```rust
// 成功状态 - 绿色
ui.global::<AppState>().set_file_status("文件读取成功: D:\\config\\mcu_config.txt".into());
ui.global::<AppState>().set_file_status_color(slint::Color::from_rgb_u8(40, 167, 69).into());

// 错误状态 - 红色  
ui.global::<AppState>().set_file_status("读取错误: 文件不存在或无法访问".into());
ui.global::<AppState>().set_file_status_color(slint::Color::from_rgb_u8(220, 53, 69).into());

// 进行中状态 - 黄色
ui.global::<AppState>().set_file_status("设备读取中...".into());
ui.global::<AppState>().set_file_status_color(slint::Color::from_rgb_u8(255, 193, 7).into());
```

## 代码结构

### Slint组件

1. **app-window.slint** - 主窗口，组合所有子组件
2. **connection-panel.slint** - 连接控制组件
3. **address-panel.slint** - 地址操作组件
4. **io-control-panel.slint** - IO控制组件
5. **file-operation-panel.slint** - 文件操作组件

### 全局状态管理

所有动态内容都通过 `AppState` 全局变量管理，包括：

```slint
export global AppState {
    // 连接状态
    in-out property <string> mcu-label;
    in-out property <string> port-label;
    in-out property <string> connect-button;
    in-out property <bool> is-connected;
    
    // 地址操作
    in-out property <string> start-address-value;
    in-out property <string> param-value;
    
    // IO状态
    in-out property <string> io1-display;
    in-out property <string> io2-display;
    
    // 文件内容
    in-out property <string> file-path;
    in-out property <string> file-content;
}
```

### 响应式布局

- **最小窗口尺寸**：800x600
- **推荐窗口尺寸**：1200x800
- **左右比例**：40:60
- **自适应缩放**：所有组件支持窗口大小变化

### 回调函数

主程序中定义了所有按钮的回调函数：

- `connect-clicked()` - 连接/断开设备
- `read-address-clicked()` / `write-address-clicked()` - 地址读写
- `io1-low-clicked()` / `io1-high-clicked()` - IO1控制
- `io2-low-clicked()` / `io2-high-clicked()` - IO2控制
- `read-file-clicked()` / `read-device-clicked()` / `config-file-clicked()` - 文件操作

## 运行程序

```bash
cargo run
```

## 后续开发

当前所有回调函数都已定义但仅输出调试信息。可以在 `src/main.rs` 中实现具体的业务逻辑，例如：

1. 串口通信逻辑
2. 文件读写操作
3. 设备配置管理
4. 状态显示更新

## 特性

- ✅ 模块化组件设计
- ✅ 响应式布局
- ✅ 全局状态管理
- ✅ 完整的事件处理
- ✅ 现代化UI设计
- ✅ 跨平台支持
