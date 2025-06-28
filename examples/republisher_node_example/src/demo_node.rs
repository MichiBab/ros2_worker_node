// Simple example of a ROS node using a simple Library Node wrapper.

use example_interfaces::{
    self as std_msgs,
    srv::{AddTwoInts, AddTwoInts_Request, AddTwoInts_Response},
};
use rclrs::ServiceOptions;
use ros2_worker_node::{RosNode, TimerOptions};

#[derive(RosNode)]
pub(crate) struct MyRosNodeExample {
    number: i64,
    number_publisher: rclrs::Publisher<std_msgs::msg::Int64>,
}

impl MyRosNodeExample {
    pub fn new(node: &rclrs::Node, topic: &str) -> Self {
        MyRosNodeExample {
            number: 0,
            number_publisher: node
                .create_publisher::<std_msgs::msg::Int64>(topic)
                .unwrap(),
        }
    }

    fn sub_callback(&mut self, msg: std_msgs::msg::Int64) {
        self.number = msg.data;
        println!("MyRosNodeExample Received number: {}", msg.data);
        println!(
            "MyRosNodeExample Current number in data stored: {}",
            self.number
        );
    }

    fn service_callback(&mut self, req: AddTwoInts_Request) -> AddTwoInts_Response {
        // Example service callback that adds both integers from the request to our number
        self.number += req.a + req.b;
        println!(
            "Service called, adding both directly, new number: {}",
            self.number
        );
        AddTwoInts_Response { sum: self.number }
    }

    pub fn new_node(node: &rclrs::Node) -> Result<RosNode<Self>, rclrs::RclrsError> {
        let payload = MyRosNodeExample::new(node, "numbers");

        let mut ros_node = RosNode::new(node, payload);

        ros_node.create_service::<AddTwoInts, _>(
            ServiceOptions::new("add_two_ints_to_number"),
            move |self_ptr: &mut Self, req: AddTwoInts_Request| self_ptr.service_callback(req),
        )?;

        ros_node.create_subscription("numbers", move |data: &mut Self, msg| {
            data.sub_callback(msg);
        })?;

        let mut _timer = ros_node.create_timer(
            TimerOptions::new(std::time::Duration::from_secs(1)).make_global(),
            |self_ptr: &mut Self| {
                self_ptr.number += 1; // Increment the number
                self_ptr
                    .number_publisher
                    .publish(&std_msgs::msg::Int64 {
                        data: self_ptr.number,
                    })
                    .unwrap();
                println!("Published number: {}", self_ptr.number);
            },
        );

        Ok(ros_node)
    }
}
