package AviationExample {
    private import AviationLibraryATA::*;
    
    part A350 : Aircraft {
        part Systems : AircraftSystem {
            part FlightControlComputer [3] : AircraftSystemsATAs::FlightControl {
                part CPU : AircraftSystemsATAs::IntegratedCircuit;
            }
            part CMS : AircraftSystemsATAs::CabinSystem {
                part GPU : AircraftSystemsATAs::IntegratedCircuit;
            }
            part Transponder : AircraftSystemsATAs::CommunicationSystem;
            part AirCondition : AircraftSystemsATAs::AirConditioning;
        }
        part Body : Structure {
            part LeftWing : StructureATAs::Wing;
            part RightWing : StructureATAs::Wing;
            part LongFusalage : StructureATAs::Fusalage;
        }
    }
}
