package DemoDASC {

    library package AviationLibraryATA {
        part def Aircraft;

        part def AircraftSystem;
        package AircraftSystemsATAs {
            part def AirConditioning;
            part def FlightControl;
            part def CabinSystem;
            part def CommunicationSystem;
            part def IntegratedCircuit;
        }
        part def Structure;
        package StructureATAs {
            part def Door;
            part def Fuselage;
            part def LandingGear;
            part def Wing;
        }
    }

    package AviationExample {
        private import AviationLibraryATA::*;

        part A350 : Aircraft {
            part Systems : AircraftSystem {
                part FCC [3] : AircraftSystemsATAs::FlightControl {
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
                part Fuselage : StructureATAs::Fuselage {
                    part Doors [10] : StructureATAs::Door;
                }
            }
        }
    }
}