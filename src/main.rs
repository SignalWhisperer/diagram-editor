use std::{cell::RefCell, collections::HashMap, rc::Rc};

use eframe::{App, CreationContext};
use egui::{Color32, Id, Ui};
use egui_snarl::{
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
    ui::{
        NodeLayout, PinInfo, PinPlacement, SnarlStyle, SnarlViewer, SnarlWidget, get_selected_nodes,
    },
};

#[derive(Clone, serde::Serialize, serde::Deserialize, Copy, PartialEq, Eq)]
enum InputKind {
    Normal,
    External,
    Internal,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Input {
    name: String,
    kind: InputKind,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            name: "Input".to_string(),
            kind: InputKind::Normal,
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Copy, PartialEq, Eq)]
enum OutputKind {
    Normal,
    External,
    Internal,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Output {
    name: String,
    kind: OutputKind,
}

impl Default for Output {
    fn default() -> Self {
        Self {
            name: "Output".to_string(),
            kind: OutputKind::Normal,
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Node {
    name: String,
    inputs: Vec<Input>,
    outputs: Vec<Output>,
    subsystem: Option<Rc<RefCell<Subsystem>>>,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            name: "Node".to_string(),
            inputs: Vec::default(),
            outputs: Vec::default(),
            subsystem: None,
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Subsystem {
    snarl: Snarl<Node>,
}

impl Default for Subsystem {
    fn default() -> Self {
        Self::new()
    }
}

impl Subsystem {
    fn new() -> Self {
        Self {
            snarl: Snarl::new(),
        }
    }
}

struct DiagramViewer {
    toplevel: Rc<RefCell<Subsystem>>,
    current: Rc<RefCell<Subsystem>>,
    previous: Vec<Rc<RefCell<Subsystem>>>,
}

impl SnarlViewer<Node> for DiagramViewer {
    fn title(&mut self, node: &Node) -> String {
        node.name.clone()
    }

    fn inputs(&mut self, node: &Node) -> usize {
        node.inputs.len()
    }

    fn outputs(&mut self, node: &Node) -> usize {
        node.outputs.len()
    }

    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut Ui,
        snarl: &mut Snarl<Node>,
    ) -> impl egui_snarl::ui::SnarlPin + 'static {
        let node = &mut snarl[pin.id.node];
        ui.add_sized(
            [200.0, 20.0],
            egui::TextEdit::singleline(&mut node.inputs[pin.id.input].name),
        );
        PinInfo::square().with_wire_color(Color32::from_rgb(255, 0, 0))
    }

    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut Ui,
        snarl: &mut Snarl<Node>,
    ) -> impl egui_snarl::ui::SnarlPin + 'static {
        let node = &mut snarl[pin.id.node];
        ui.add_sized(
            [200.0, 20.0],
            egui::TextEdit::singleline(&mut node.outputs[pin.id.output].name),
        );
        PinInfo::square().with_wire_color(Color32::from_rgb(0, 0, 255))
    }

    fn show_header(
        &mut self,
        node_id: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut Ui,
        snarl: &mut Snarl<Node>,
    ) {
        let node = &mut snarl[node_id];
        ui.add_sized([200.0, 20.0], egui::TextEdit::singleline(&mut node.name));
    }

    fn drop_inputs(&mut self, pin: &InPin, snarl: &mut Snarl<Node>) {
        if snarl.drop_inputs(pin.id) == 0
            && let Some(node) = snarl.get_node_mut(pin.id.node)
        {
            // TODO: doing it this way crashes, we need to schedule the removal
            node.inputs.remove(pin.id.input);
        }
    }

    fn drop_outputs(&mut self, pin: &OutPin, snarl: &mut Snarl<Node>) {
        if snarl.drop_outputs(pin.id) == 0
            && let Some(node) = snarl.get_node_mut(pin.id.node)
        {
            // TODO: doing it this way crashes, we need to schedule the removal
            node.outputs.remove(pin.id.output);
        }
    }

    fn has_node_menu(&mut self, _node: &Node) -> bool {
        true
    }

    fn show_node_menu(
        &mut self,
        node_id: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut Ui,
        snarl: &mut Snarl<Node>,
    ) {
        let node = &mut snarl[node_id];

        ui.label("Node menu");
        ui.separator();

        if ui.button("Add Input").clicked() {
            node.inputs.push(Input::default());
            ui.close();
        }

        if ui.button("Add Output").clicked() {
            node.outputs.push(Output::default());
            ui.close();
        }

        ui.separator();

        if ui.button("Enter Subsystem").clicked() {
            self.previous.push(self.current.clone());
            self.current = if let Some(subsystem) = node.subsystem.as_ref() {
                subsystem.clone()
            } else {
                Rc::new(RefCell::new(Subsystem::default()))
            };
        }

        ui.separator();
        ui.separator();

        if ui.button("Remove Node").clicked() {
            snarl.remove_node(node_id);
            ui.close();
        }
    }

    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<Node>) -> bool {
        true
    }

    fn show_graph_menu(&mut self, pos: egui::Pos2, ui: &mut Ui, snarl: &mut Snarl<Node>) {
        ui.label("Diagram Menu");
        ui.separator();

        if ui.button("Add Node").clicked() {
            snarl.insert_node(pos, Node::default());
            ui.close();
        }

        let selected = get_selected_nodes(Id::new("diagram"), ui.ctx());

        if ui
            .add_enabled(
                !selected.is_empty(),
                egui::Button::new("Convert To Subsystem"),
            )
            .clicked()
        {
            // Ports that are not connected internally become part of the subsytem ports
            // and are internally connected to an "external" port.
            // If they were connected externally, we re-create this connection once again.
            // If they were unconnected, we leave them unconnected externally.

            let mut subsystem = Subsystem::default();

            // List all the relevant connections
            let wires = snarl
                .wires()
                .filter(|(pin_out, pin_in)| {
                    selected.contains(&pin_in.node) || selected.contains(&pin_out.node)
                })
                .collect::<Vec<_>>();

            let internal_wires = wires
                .iter()
                .filter(|(pin_out, pin_in)| {
                    selected.contains(&pin_in.node) && selected.contains(&pin_out.node)
                })
                .collect::<Vec<_>>();
            let external_inputs = wires
                .iter()
                .filter(|(pin_out, pin_in)| {
                    selected.contains(&pin_in.node) && !selected.contains(&pin_out.node)
                })
                .collect::<Vec<_>>();
            let external_outputs = wires
                .iter()
                .filter(|(pin_out, pin_in)| {
                    !selected.contains(&pin_in.node) && selected.contains(&pin_out.node)
                })
                .collect::<Vec<_>>();

            // Create external input nodes internally
            let external_input_names = external_inputs
                .iter()
                .map(|(_, pin_in)| snarl[pin_in.node].inputs[pin_in.input].name.clone())
                .collect::<Vec<_>>();

            let external_input_nodes = external_input_names
                .iter()
                .map(|name| Output {
                    name: name.clone(),
                    kind: OutputKind::External,
                })
                .enumerate()
                .map(|(n, output)| {
                    subsystem.snarl.insert_node(
                        [0.0, n as f32 * 50.0].into(),
                        Node {
                            name: format!("Ext{}", n + 1),
                            inputs: Vec::default(),
                            outputs: vec![output],
                            subsystem: None,
                        },
                    )
                })
                .collect::<Vec<_>>();

            // Create external output nodes internally
            let external_output_names = external_outputs
                .iter()
                .map(|(pin_out, _)| snarl[pin_out.node].outputs[pin_out.output].name.clone())
                .collect::<Vec<_>>();

            let external_output_nodes = external_output_names
                .iter()
                .map(|name| Input {
                    name: name.clone(),
                    kind: InputKind::External,
                })
                .enumerate()
                .map(|(n, input)| {
                    subsystem.snarl.insert_node(
                        [100.0, n as f32 * 50.0].into(),
                        Node {
                            name: format!("Ext{}", n + 1),
                            inputs: vec![input],
                            outputs: Vec::default(),
                            subsystem: None,
                        },
                    )
                })
                .collect::<Vec<_>>();

            // Map the old node IDs to the new ones
            let mut node_map: HashMap<NodeId, NodeId> = HashMap::default();
            for node_id in selected {
                let Some(node) = snarl.get_node_info(node_id) else {
                    continue;
                };
                let new_node_id = subsystem
                    .snarl
                    .insert_node(node.pos, snarl.remove_node(node_id));
                node_map.insert(node_id, new_node_id);
            }

            // Re-create the internal connections
            internal_wires
                .into_iter()
                .filter_map(|(pin_out, pin_in)| {
                    Some((
                        OutPinId {
                            node: *node_map.get(&pin_out.node)?,
                            output: pin_out.output,
                        },
                        InPinId {
                            node: *node_map.get(&pin_in.node)?,
                            input: pin_in.input,
                        },
                    ))
                })
                .for_each(|(pin_out, pin_in)| {
                    subsystem.snarl.connect(pin_out, pin_in);
                });

            // Create the external input connections internally
            external_inputs
                .iter()
                .enumerate()
                .map(|(n, (_, pin_in))| {
                    (
                        OutPinId {
                            node: external_input_nodes[n],
                            output: 0,
                        },
                        InPinId {
                            node: *node_map
                                .get(&pin_in.node)
                                .expect("Old input pin node is mapped to new node"),
                            input: pin_in.input,
                        },
                    )
                })
                .for_each(|(pin_out, pin_in)| {
                    subsystem.snarl.connect(pin_out, pin_in);
                });

            // Create the external output connections internally
            external_outputs
                .iter()
                .enumerate()
                .map(|(n, (pin_out, _))| {
                    (
                        OutPinId {
                            node: *node_map
                                .get(&pin_out.node)
                                .expect("Old output pin node is mapped to new node"),
                            output: pin_out.output,
                        },
                        InPinId {
                            node: external_output_nodes[n],
                            input: 0,
                        },
                    )
                })
                .for_each(|(pin_out, pin_in)| {
                    subsystem.snarl.connect(pin_out, pin_in);
                });

            // Create the external subsystem node
            let mut new_node = Node {
                name: "Subsystem".to_string(),
                inputs: external_input_names
                    .iter()
                    .map(|name| Input {
                        name: name.clone(),
                        kind: InputKind::Internal,
                    })
                    .collect(),
                outputs: external_output_names
                    .iter()
                    .map(|name| Output {
                        name: name.clone(),
                        kind: OutputKind::Internal,
                    })
                    .collect(),
                subsystem: None,
            };

            // Add the unconnected inputs
            subsystem
                .snarl
                .node_ids()
                .flat_map(|(node_id, node)| {
                    node.inputs
                        .iter()
                        .enumerate()
                        .filter_map(|(n, input)| {
                            let pin = subsystem.snarl.in_pin(InPinId {
                                node: node_id,
                                input: n,
                            });
                            if !pin.remotes.is_empty() {
                                None
                            } else {
                                Some((
                                    node_id,
                                    n,
                                    Input {
                                        name: input.name.clone(),
                                        kind: InputKind::Internal,
                                    },
                                ))
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
                .into_iter()
                .enumerate()
                .for_each(|(n, (node_id, port, input))| {
                    // Create new internal input nodes
                    let input_node_id = subsystem.snarl.insert_node(
                        [0.0, n as f32 * -150.0].into(),
                        Node {
                            name: format!("ExtUC{}", n + 1),
                            inputs: Vec::default(),
                            outputs: vec![Output {
                                name: input.name.clone(),
                                kind: OutputKind::External,
                            }],
                            subsystem: None,
                        },
                    );

                    subsystem.snarl.connect(
                        OutPinId {
                            node: input_node_id,
                            output: 0,
                        },
                        InPinId {
                            node: node_id,
                            input: port,
                        },
                    );

                    // Add it to the subsystem block
                    new_node.inputs.push(input);
                });

            // Add the unconnected outputs
            subsystem
                .snarl
                .node_ids()
                .flat_map(|(node_id, node)| {
                    node.outputs
                        .iter()
                        .enumerate()
                        .filter_map(|(n, output)| {
                            let pin = subsystem.snarl.out_pin(OutPinId {
                                node: node_id,
                                output: n,
                            });
                            if !pin.remotes.is_empty() {
                                None
                            } else {
                                Some((
                                    node_id,
                                    n,
                                    Output {
                                        name: output.name.clone(),
                                        kind: OutputKind::Internal,
                                    },
                                ))
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
                .into_iter()
                .enumerate()
                .for_each(|(n, (node_id, port, output))| {
                    // Create new internal output nodes
                    let output_node_id = subsystem.snarl.insert_node(
                        [300.0, n as f32 * -150.0].into(),
                        Node {
                            name: format!("ExtOutUC{}", n + 1),
                            inputs: vec![Input {
                                name: output.name.clone(),
                                kind: InputKind::External,
                            }],
                            outputs: Vec::default(),
                            subsystem: None,
                        },
                    );

                    subsystem.snarl.connect(
                        OutPinId {
                            node: node_id,
                            output: port,
                        },
                        InPinId {
                            node: output_node_id,
                            input: 0,
                        },
                    );

                    // Add it to the subsystem block
                    new_node.outputs.push(output);
                });

            new_node.subsystem = Some(Rc::new(RefCell::new(subsystem)));
            let new_node_id = snarl.insert_node(pos, new_node);

            // Connect the previously connected inputs and outputs to the new subsystem node
            external_inputs
                .iter()
                .enumerate()
                .map(|(n, (pin_out, _))| {
                    (
                        pin_out,
                        InPinId {
                            node: new_node_id,
                            input: n,
                        },
                    )
                })
                .for_each(|(pin_out, pin_in)| {
                    snarl.connect(*pin_out, pin_in);
                });
            external_outputs
                .iter()
                .enumerate()
                .map(|(n, (_, pin_in))| {
                    (
                        OutPinId {
                            node: new_node_id,
                            output: n,
                        },
                        pin_in,
                    )
                })
                .for_each(|(pin_out, pin_in)| {
                    snarl.connect(pin_out, *pin_in);
                });

            ui.close();
        }

        if !self.previous.is_empty() {
            ui.separator();
            ui.separator();
            if ui.button("Go Up One Level").clicked() {
                if let Some(previous) = self.previous.pop() {
                    self.current = previous;
                }

                ui.close();
            }
        }
    }
}

struct DiagramApp {
    viewer: DiagramViewer,
    style: SnarlStyle,
}

const fn default_style() -> SnarlStyle {
    SnarlStyle {
        node_layout: Some(NodeLayout::coil()),
        pin_placement: Some(PinPlacement::Edge),
        pin_size: Some(7.0),
        node_frame: Some(egui::Frame {
            inner_margin: egui::Margin::same(8),
            outer_margin: egui::Margin {
                left: 0,
                right: 0,
                top: 0,
                bottom: 4,
            },
            corner_radius: egui::CornerRadius::same(8),
            fill: egui::Color32::from_gray(30),
            stroke: egui::Stroke::NONE,
            shadow: egui::Shadow::NONE,
        }),
        collapsible: Some(false),
        bg_frame: Some(egui::Frame {
            inner_margin: egui::Margin::ZERO,
            outer_margin: egui::Margin::same(2),
            corner_radius: egui::CornerRadius::ZERO,
            fill: egui::Color32::from_gray(40),
            stroke: egui::Stroke::NONE,
            shadow: egui::Shadow::NONE,
        }),
        ..SnarlStyle::new()
    }
}

impl DiagramApp {
    pub fn new(cx: &CreationContext) -> Self {
        egui_extras::install_image_loaders(&cx.egui_ctx);

        let toplevel = cx.storage.map_or_else(Subsystem::new, |storage| {
            storage
                .get_string("toplevel")
                .and_then(|subsystem| serde_json::from_str(&subsystem).ok())
                .unwrap_or_default()
        });

        let style = cx.storage.map_or_else(default_style, |storage| {
            storage
                .get_string("style")
                .and_then(|style| serde_json::from_str(&style).ok())
                .unwrap_or_else(default_style)
        });

        let system = Rc::new(RefCell::new(toplevel));

        Self {
            viewer: DiagramViewer {
                toplevel: system.clone(),
                current: system,
                previous: Vec::default(),
            },
            style,
        }
    }
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Diagram",
        native_options,
        Box::new(|cx| Ok(Box::new(DiagramApp::new(cx)))),
    )
}

impl App for DiagramApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.add_space(16.0);

                egui::widgets::global_theme_preference_switch(ui);
            });
        });

        egui::SidePanel::left("style").show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui_probe::Probe::new(&mut self.style).show(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            SnarlWidget::new()
                .id(Id::new("diagram"))
                .style(self.style)
                .show(
                    &mut self.viewer.current.clone().borrow_mut().snarl,
                    &mut self.viewer,
                    ui,
                );
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let system = serde_json::to_string(&self.viewer.toplevel).unwrap();
        storage.set_string("toplevel", system);

        let style = serde_json::to_string(&self.style).unwrap();
        storage.set_string("style", style);
    }
}
