use std::{fmt::Debug, marker::PhantomData};

use super::formater::FormaterExt;
use super::sink::SinkExt;
use cfapi::binding::MessageEvent;

use cfapi::message_event::MessageEventHandlerExt;
use crossbeam_queue::ArrayQueue;
use tracing::{error, info};
// use crossbeam_utils::thread::scope;
use super::convertor::Convertor;
use crossbeam_channel::bounded;

pub struct PipeQueueMessageHandler<C, F, R>
where
    C: Convertor + Send + Sync,
    F: FormaterExt<C::Out> + Send + Sync,
    R: SinkExt<C::Out> + Send + Sync,
    C::Out: Send + Sync + Debug,
{
    convertor: C,
    // formater: F,
    // sink: R,
    // queue: &'a ArrayQueue<C::Out>,
    recv: crossbeam_channel::Receiver<C::Out>,
    send: crossbeam_channel::Sender<C::Out>,
    _formater: PhantomData<F>,
    _sink: PhantomData<R>,
}

impl<C, F, R> PipeQueueMessageHandler<C, F, R>
where
    C: Convertor + Send + Sync,
    F: FormaterExt<C::Out> + Send + Sync + Default,
    R: SinkExt<C::Out> + Send + Sync + Default,
    C::Out: Send + Sync + Debug,
{
    // pub fn new(convertor: C, formater: F, sink: R, size: usize) -> Self {
    //     let (s, r) = bounded(size);
    //     Self {
    //         convertor,
    //         formater,
    //         sink,
    //         recv: r,
    //         send: s,
    //         // queue: ArrayQueue::new(size),
    //         // queue: &queue,
    //         // ph: PhantomData,
    //     }
    // }

    pub fn new(convertor: C, size: usize) -> Self 
    where 
        F: FormaterExt<C::Out> + Send + Sync + Default,
        R: SinkExt<C::Out> + Send + Sync + Default,
    {
        let (s, r) = bounded(size);
        Self {
            convertor,
            // formater,
            // sink,
            recv: r,
            send: s,
            _formater: PhantomData,
            _sink: PhantomData,
        }
    }

    // pub fn exec(&self) {
    //     match self.recv.recv() {
    //         Ok(data) => {
    //             self.sink.exec(&data, &self.formater);
    //         }
    //         Err(_) => {
    //             error!("channel is empty");
    //         }
    //     }
    //     // if !self.queue.is_empty() {
    //     //     let data = self.queue.pop();
    //     //     match data {
    //     //         Some(data) => {
    //     //             self.sink.exec(&data, &self.formater);
    //     //         }
    //     //         None => {
    //     //             info!("queue is empty");
    //     //         }
    //     //     }
    //     // }
    // }

    // pub fn exec_loop(&self) {
    //     loop {
    //         self.exec();
    //     }
    // }

    pub fn exec_loop_th(&self)
    where
        <C as Convertor>::Out: 'static,
        F: FormaterExt<C::Out> + Send + Sync + Default,
        R: SinkExt<C::Out> + Send + Sync + Default,
    {
        let recv = self.recv.clone();
        std::thread::spawn(move || {
            let formater = F::default();
            let mut sink = R::default();
            // let recv = recv.clone();
            loop {
                match recv.recv() {
                    Ok(data) => {
                        // info!("data: {:?}", data);
                        // println!("data: {:?}", data);
                        sink.exec(&data, &formater);
                    }
                    Err(_) => {
                        error!("channel is empty");
                    }
                }
            }
            // self.exec_loop();
        });
        // scope(|scope| {
        //     println!("start exec_loop_th");
        //     scope.spawn(|_| {
        //         self.exec_loop();
        //     });
        //     println!("started exec_loop_th");
        // });
        // .unwrap()
    }

    pub fn get_queue_size(&self) -> usize {
        self.recv.len()
    }
}

impl<C, F, R> MessageEventHandlerExt for PipeQueueMessageHandler<C, F, R>
where
    C: Convertor + Send + Sync,
    F: FormaterExt<C::Out> + Send + Sync,
    R: SinkExt<C::Out> + Send + Sync,
    C::Out: Send + Sync + Debug,
{
    fn on_message_event(&mut self, event: &MessageEvent) {
        if event.getSource() == autocxx::c_int(0) {
            return;
        }
        let data = self.convertor.convert(event);
        match data {
            Some(data) => {
                match self.send.send(data) {
                    Ok(_) => {
                        // info!("send");
                    }
                    Err(_) => {
                        error!("channel is full");
                    }
                }
            }
            None => {}
        }
    }
}
