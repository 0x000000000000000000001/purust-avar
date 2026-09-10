module Test.Extra where

import Prelude

import Data.Either (Either(..))
import Data.Maybe (Maybe(..))
import Effect (Effect)
import Effect.AVar as AVar
import Effect.Console (log)
import Effect.Exception (Error, error, message, throw, try)
import Effect.Ref as Ref
import Test.Assert (assert, assertEqual)

record :: Ref.Ref String -> String -> Effect Unit
record ref value = Ref.modify_ (_ <> value) ref

received :: Ref.Ref String -> String -> Either Error Int -> Effect Unit
received ref label = case _ of
  Left err -> record ref (label <> "!" <> message err <> ";")
  Right value -> record ref (label <> show value <> ";")

run :: Effect Unit
run = do
  do
    var <- AVar.empty
    empty <- AVar.status var
    assert (AVar.isEmpty empty && not (AVar.isFilled empty) && not (AVar.isKilled empty))
    _ <- AVar.tryPut 42 var
    filled <- AVar.status var
    case filled of
      AVar.Filled value -> assertEqual { actual: value, expected: 42 }
      _ -> assert false
    AVar.kill (error "first") var
    AVar.kill (error "second") var
    killed <- AVar.status var
    case killed of
      AVar.Killed err -> assertEqual { actual: message err, expected: "first" }
      _ -> assert false
    canPut <- AVar.tryPut 9 var
    canRead <- AVar.tryRead var
    canTake <- AVar.tryTake var
    assert (not canPut && canRead == Nothing && canTake == Nothing)
    log "[OK] status transitions and first kill wins"
  do
    var <- AVar.empty
    trace <- Ref.new ""
    _ <- AVar.take var (received trace "t1:")
    _ <- AVar.take var (received trace "t2:")
    _ <- AVar.take var (received trace "t3:")
    _ <- AVar.read var (received trace "r1:")
    _ <- AVar.read var (received trace "r2:")
    _ <- AVar.put 10 var (\_ -> record trace "p1;")
    _ <- AVar.put 20 var (\_ -> record trace "p2;")
    _ <- AVar.put 30 var (\_ -> record trace "p3;")
    actual <- Ref.read trace
    assertEqual { actual, expected: "r1:10;r2:10;t1:10;p1;t2:20;p2;t3:30;p3;" }
    log "[OK] FIFO takes and read broadcast before take and put callbacks"
  do
    var <- AVar.new 0
    trace <- Ref.new ""
    _ <- AVar.put 1 var (\_ -> record trace "p1;")
    _ <- AVar.put 2 var (\_ -> record trace "p2;")
    _ <- AVar.put 3 var (\_ -> record trace "p3;")
    first <- AVar.tryTake var
    second <- AVar.tryTake var
    third <- AVar.tryTake var
    fourth <- AVar.tryTake var
    assertEqual { actual: [first, second, third, fourth], expected: [Just 0, Just 1, Just 2, Just 3] }
    actual <- Ref.read trace
    assertEqual { actual, expected: "p1;p2;p3;" }
    log "[OK] FIFO puts refill after each take"
  do
    var <- AVar.empty
    trace <- Ref.new ""
    cancel <- Ref.new (pure unit)
    _ <- AVar.read var \value -> do
      received trace "first:" value
      action <- Ref.read cancel
      action
      snapshot <- AVar.tryRead var
      assertEqual { actual: snapshot, expected: Just 7 }
      void $ AVar.take var (received trace "nested:")
    cancelSecond <- AVar.read var (received trace "BAD:")
    Ref.write cancelSecond cancel
    _ <- AVar.read var (received trace "last:")
    _ <- AVar.put 7 var (\_ -> record trace "put;")
    actual <- Ref.read trace
    assertEqual { actual, expected: "first:7;last:7;put;nested:7;" }
    log "[OK] callbacks can read, cancel another waiter and enqueue a take"
  do
    var <- AVar.empty
    trace <- Ref.new ""
    cancel <- AVar.take var (received trace "canceled:")
    cancel
    cancel
    _ <- AVar.tryPut 8 var
    completed <- AVar.read var (received trace "read:")
    completed
    completed
    value <- AVar.tryRead var
    actual <- Ref.read trace
    assertEqual { actual, expected: "read:8;" }
    assertEqual { actual: value, expected: Just 8 }
    log "[OK] cancellation is idempotent before and after completion"
  do
    full <- AVar.new 0
    trace <- Ref.new ""
    let
      failed :: forall a. String -> Either Error a -> Effect Unit
      failed label = case _ of
        Left err -> record trace (label <> message err <> ";")
        Right _ -> record trace "BAD;"
    _ <- AVar.put 1 full (failed "p1:")
    _ <- AVar.put 2 full (failed "p2:")
    AVar.kill (error "killed") full
    _ <- AVar.put 3 full (failed "future-put:")
    _ <- AVar.read full (failed "future-read:")
    _ <- AVar.take full (failed "future-take:")
    actual <- Ref.read trace
    assertEqual { actual, expected: "p1:killed;p2:killed;future-put:killed;future-read:killed;future-take:killed;" }
    log "[OK] kill rejects queued puts and every future callback"
  do
    var <- AVar.empty
    trace <- Ref.new ""
    _ <- AVar.read var (\_ -> throw "callback failure")
    _ <- AVar.read var (received trace "survivor:")
    result <- try (AVar.tryPut 9 var)
    case result of
      Left err -> assertEqual { actual: message err, expected: "callback failure" }
      Right _ -> assert false
    actual <- Ref.read trace
    assertEqual { actual, expected: "survivor:9;" }
    value <- AVar.tryTake var
    _ <- AVar.tryPut 10 var
    next <- AVar.tryTake var
    assertEqual { actual: [value, next], expected: [Just 9, Just 10] }
    log "[OK] callback failure does not strand waiters or poison the AVar"
  do
    let create = AVar.new 1
    first <- create
    second <- create
    _ <- AVar.tryTake first
    stillFilled <- AVar.tryRead second
    assertEqual { actual: stillFilled, expected: Just 1 }
    trace <- Ref.new ""
    let enqueue = AVar.take first (received trace "take:")
    cancelFirst <- enqueue
    _ <- enqueue
    cancelFirst
    _ <- AVar.tryPut 2 first
    actual <- Ref.read trace
    assertEqual { actual, expected: "take:2;" }
    log "[OK] replay creates independent AVars and independent subscriptions"
