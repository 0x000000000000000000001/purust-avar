module Test.Runner where

import Prelude
import Effect (Effect)
import Test.Main as Original
import Test.Extra as Extra
import Test.Probe (watchdog)

main :: Effect Unit
main = do
  watchdog
  Original.main
  Extra.run
