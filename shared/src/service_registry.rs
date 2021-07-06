//! A general purpose registry for a service's components.
//!
//! The registry allows adding new components as well as linking them so that
//! they can be accessed as a trait object.

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Arc,
};

#[derive(Default)]
pub struct ServiceRegistry {
    instances: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl ServiceRegistry {
    pub fn register_as<T, U>(&mut self) -> &mut Self
    where
        T: ServiceInitializable + ServiceLinkable<U> + Send + Sync + 'static,
        U: Send + Sync + ?Sized + 'static,
    {
        self.register::<T>().link::<T, U>()
    }

    pub fn register<T>(&mut self) -> &mut Self
    where
        T: ServiceInitializable + Send + Sync + 'static,
    {
        let instance = T::init(&self);
        self.add(instance)
    }

    pub fn add<T>(&mut self, instance: T) -> &mut Self
    where
        T: Send + Sync + 'static,
    {
        // This is counter intuitive, but we don't store the `Arc` directly in
        // our instance map because we require double boxing. This is because a
        // single `Arc<T>` can be shared with multiple `Arc<dyn X>` trait
        // objects, and we need to keep a separate entry in the map for each of
        // them for them to hold different v-tables.
        self.instances
            .insert(TypeId::of::<Arc<T>>(), Box::new(Arc::new(instance)));
        self
    }

    pub fn link<T, U>(&mut self) -> &mut Self
    where
        T: ServiceLinkable<U> + Send + Sync + 'static,
        U: Send + Sync + ?Sized + 'static,
    {
        self.instances
            .insert(TypeId::of::<Arc<U>>(), Box::new(self.get::<T>().as_link()));
        self
    }

    pub fn get<T>(&self) -> Arc<T>
    where
        T: Send + Sync + ?Sized + 'static,
    {
        self.try_get().expect("missing service")
    }

    pub fn try_get<T>(&self) -> Option<Arc<T>>
    where
        T: Send + Sync + ?Sized + 'static,
    {
        Some(
            self.instances
                .get(&TypeId::of::<Arc<T>>())?
                .downcast_ref::<Arc<T>>()
                .expect("incorrectly registered service")
                .clone(),
        )
    }
}

pub trait ServiceInitializable: Sized {
    fn init(registry: &ServiceRegistry) -> Self;
}

impl<T> ServiceInitializable for T
where
    T: Default + Sized,
{
    fn init(_: &ServiceRegistry) -> Self {
        T::default()
    }
}

pub trait ServiceLinkable<T: ?Sized> {
    fn as_link(self: Arc<Self>) -> Arc<T>;
}

#[macro_export]
macro_rules! impl_service_linkable {
    ($impl:ident => $trait:ident) => {
        impl ServiceLinkable<dyn $trait> for $impl {
            fn as_link(self: Arc<Self>) -> Arc<dyn $trait> {
                self
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bad_token::{BadTokenDetecting, MockBadTokenDetecting, TokenQuality},
        gas_price_estimation::FakeGasPriceEstimator,
        price_estimate::{BaselinePriceEstimator, PriceEstimating},
        sources::uniswap::pool_fetching::{MockPoolFetching, PoolFetching},
    };
    use ethcontract::H160;
    use gas_estimation::GasPriceEstimating;
    use model::order::OrderKind;
    use std::collections::HashSet;

    trait Fooable: Send + Sync {
        fn foo(&self) -> i32;
    }

    struct Foo;
    impl Fooable for Foo {
        fn foo(&self) -> i32 {
            42
        }
    }
    impl ServiceInitializable for Foo {
        fn init(_: &ServiceRegistry) -> Self {
            Foo
        }
    }
    impl_service_linkable!(Foo => Fooable);

    struct Bar(Arc<dyn Fooable>);
    impl Bar {
        fn magic(&self) -> i32 {
            (self.0.foo() * 191) / 6
        }
    }
    impl ServiceInitializable for Bar {
        fn init(registry: &ServiceRegistry) -> Self {
            Bar(registry.get())
        }
    }

    #[test]
    fn can_register_and_get() {
        let mut registry = ServiceRegistry::default();
        registry
            // Register a `Foo` implementation for a `Fooable` component.
            .register_as::<Foo, dyn Fooable>()
            // Register a concrete `Bar` component.
            .register::<Bar>();

        assert_eq!(registry.get::<dyn Fooable>().foo(), 42);
        assert_eq!(registry.get::<Bar>().magic(), 1337);
    }

    impl_service_linkable!(MockPoolFetching => PoolFetching);
    impl_service_linkable!(FakeGasPriceEstimator => GasPriceEstimating);
    impl_service_linkable!(MockBadTokenDetecting => BadTokenDetecting);
    impl_service_linkable!(BaselinePriceEstimator => PriceEstimating);

    // This would represent the command line arguments.
    struct Options {
        base_tokens: HashSet<H160>,
        native_token: H160,
    }

    impl ServiceInitializable for BaselinePriceEstimator {
        fn init(registry: &ServiceRegistry) -> Self {
            let options = registry.get::<Options>();
            Self::new(
                registry.get(),
                registry.get(),
                options.base_tokens.clone(),
                registry.get(),
                options.native_token,
            )
        }
    }

    #[tokio::test]
    async fn register_balancer_component_with_mocks() {
        let mut token_detector = MockBadTokenDetecting::new();
        token_detector
            .expect_detect()
            .returning(|_| Ok(TokenQuality::Good));

        let mut registry = ServiceRegistry::default();
        registry
            .add(Options {
                base_tokens: Default::default(),
                native_token: Default::default(),
            })
            .add(token_detector)
            .link::<MockBadTokenDetecting, dyn BadTokenDetecting>()
            .register_as::<MockPoolFetching, dyn PoolFetching>()
            .register_as::<FakeGasPriceEstimator, dyn GasPriceEstimating>()
            .register_as::<BaselinePriceEstimator, dyn PriceEstimating>();

        let estimator = registry.get::<dyn PriceEstimating>();
        assert_eq!(
            estimator
                .estimate_price(H160::zero(), H160::zero(), 1.into(), OrderKind::Sell)
                .await
                .unwrap(),
            num::one(),
        );
    }
}
