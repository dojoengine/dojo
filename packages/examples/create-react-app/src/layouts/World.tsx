import { BottomNavigation as BottomNavigationContainer } from "../containers/BottomNavigation";
import { TopNavigation as TopNavigationContainer } from "../containers/TopNavigation";
import { RealmDetails } from "../modules/RealmDetails";

export const World = () => {
  return (
    <div className="absolute top-0 z-20 w-screen h-screen z-100">
      {/* Top nav */}
      <TopNavigationContainer>
        <RealmDetails />
      </TopNavigationContainer>

      {/* Bottom nav */}
      <BottomNavigationContainer>
        <RealmDetails />
      </BottomNavigationContainer>
    </div>
  );
};
