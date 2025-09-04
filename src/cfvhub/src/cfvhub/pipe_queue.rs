use std::{fmt::Debug, marker::PhantomData};

use super::formater::FormaterExt;
use super::sink::SinkExt;
use cfapi::binding::MessageEvent;

use cfapi::message_event::MessageEventHandlerExt;
use tracing::{error, info};
// use crossbeam_utils::thread::scope;
use super::convertor::Convertor;
use crossbeam_channel::{bounded, unbounded, TrySendError};
use minitrace::prelude::{LocalSpan, Span, SpanContext};

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
    recv_back: crossbeam_channel::Receiver<C::Out>,
    send_back: crossbeam_channel::Sender<C::Out>,
    // recv_event: crossbeam_channel::Receiver<MessageEvent>,
    // send_event: crossbeam_channel::Sender<MessageEvent>,
    n: usize,
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

    pub fn new(convertor: C, size: usize, n: usize) -> Self
    where
        F: FormaterExt<C::Out> + Send + Sync + Default,
        R: SinkExt<C::Out> + Send + Sync + Default,
    {
        let (send, recv) = bounded(size);
        let (send_back, recv_back) = unbounded();
        // let (send_event, recv_event) = unbounded();
        Self {
            convertor,
            // formater,
            // sink,
            recv,
            send,
            recv_back,
            send_back,
            // recv_event,
            // send_event,
            n,
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
        for i in 0..self.n {
            let recv = self.recv.clone();
            // tracing::info!("exec_loop_th: {}", i);
            let id = i.to_string();
            std::thread::spawn(move || {
                let formater = F::default();
                let mut sink = R::build(&id);
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
            });
        }
        for i in 0..self.n {
            let recv = self.recv_back.clone();
            let id = format!("b{}", i);
            std::thread::spawn(move || {
                let formater = F::default();
                let mut sink = R::build(&id);
                // let recv = recv.clone();
                loop {
                    match recv.recv() {
                        Ok(data) => {
                            // info!("data: {:?}", data);
                            // println!("data: {:?}", data);
                            // sink.exec(&data, &formater);
                        }
                        Err(_) => {
                            error!("channel is empty");
                        }
                    }
                }
            });
        }
        let recv = self.recv.clone();
        let recv_back = self.recv_back.clone();
        std::thread::spawn(move || {
            loop {
                let queue_size = recv.len();
                let backup_queue_size = recv_back.len();
                info!("queue size: {} backup queue size: {}", queue_size, backup_queue_size);
                std::thread::sleep(std::time::Duration::from_secs(60));
            }
        });

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
        // self.send_event.send(event);
        let root = Span::root("on_msg", SpanContext::random());
        let _guard = root.set_local_parent();
        let data = self.convertor.convert(event);
        match data {
            Some(data) => {
                let _g = LocalSpan::enter_with_local_parent("send_data");
                match self.send.try_send(data) {
                    Ok(_) => {
                        // info!("send");
                    }
                    Err(TrySendError::Full(data)) => {
                        // send full notifiy
                        match self.send_back.send(data) {
                            Ok(_) => {}
                            Err(e) => {
                                error!("backup channel error {:?}", e);
                            }
                        }
                        error!("channel is full");
                    }
                    Err(TrySendError::Disconnected(_)) => {
                        error!("channel is disconnected");
                    }
                }
            }
            None => {}
        }
    }
}
