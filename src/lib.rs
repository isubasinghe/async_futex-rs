use std::{future::Future, marker::PhantomData, pin::Pin, sync::{Arc, atomic::{Ordering::SeqCst, AtomicU64}}, task::{Context, Poll, Waker}};
use crossbeam_queue::SegQueue;

static WAITING: u64 = 0;


#[derive(Debug, Clone)]
struct SharedMutexData {
    queue: Arc<SegQueue<(Waker, u64)>>,
    state: Arc<AtomicU64>
}

impl SharedMutexData {
    fn new() -> Self {
        SharedMutexData { queue: Arc::new(SegQueue::new()), state: Arc::new(AtomicU64::new(WAITING)) }
    }
}

#[derive(Debug)]
pub struct Mutex<T> {
    number: Option<u64>,
    shared: Arc<SharedMutexData>,
    state: PhantomData<T>,
}

pub trait LockingState {}
pub trait UnlockingState {}

pub struct LockingFuture {
    eventual_state: Mutex<UnlockingMode>
}

impl Future for LockingFuture {
    type Output = Mutex<UnlockingMode>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
       let waker = cx.waker().clone();
       let id = self.eventual_state.shared.state.fetch_add(1, SeqCst);
        self.eventual_state.shared.queue.push((waker, id));

       let res = self.eventual_state.shared.state.compare_exchange(WAITING, id, SeqCst, SeqCst);
       match res {
            Ok(_) => {
                // Safe since we have a guarantee that (waker, id) have been pushed at least
                let (waker, _) = self.eventual_state.shared.queue.pop().unwrap();
                waker.wake();
            },
            Err(_) => {
                return Poll::Pending;
            }
       };

       Poll::Pending
    }    
}

pub enum MutexError {
    WaitingForAccess
}


impl <L> Mutex<L>
  where
    L: LockingState
{
    
    fn new() -> Mutex<LockingMode> {
        Mutex { number: None, shared: Arc::new(SharedMutexData::new()), state: PhantomData::<LockingMode> }
    }

    fn lock(&self) -> impl Future<Output=Mutex<UnlockingMode>> {
        let shared = self.shared.clone();
        let number = self.number.clone();
        let m = Mutex { number, shared, state: PhantomData::<UnlockingMode> };
        LockingFuture { eventual_state: m }
    }

    fn clone(&self) -> Mutex<LockingMode> {
        let shared = self.shared.clone();
        let number = self.number.clone();

        Mutex { number, shared, state: PhantomData::<LockingMode> }
    }
}


impl <U> Mutex<U>
  where
    U: UnlockingState
{
    // TODO: 
    // Can we have a Mutex<UnlockingMode> but also have already unlocked the mutex?
    // I think so since the mutex can be cloned? We do have one advantage
    // When unlock() is reachable we do have the guarantee that we have satisfied mutual exclusion
    // We really need to prove these properties and show that ME holds in all honesty. 
    fn unlock(&self) -> Result<Mutex<LockingMode>, MutexError> {
        let shared = self.shared.clone();
        let number = self.number.clone(); 

        Ok(Mutex { number, shared, state: PhantomData::<LockingMode> })
    }
}


#[derive(Debug)]
pub enum LockingMode {}

#[derive(Debug)]
pub enum UnlockingMode {}

impl LockingState for LockingMode {}
impl UnlockingState for UnlockingMode {}



